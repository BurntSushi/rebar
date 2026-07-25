#!/usr/bin/env ruby

require 'benchmark'

# Represents a single KLV (Key-Length-Value) item from the input stream
class KLV
  attr_reader :key, :value

  def initialize(key, value)
    @key = key
    @value = value
  end

  # Parse a single KLV item from the given bytes
  # Format is key:length:value\n where value is exactly length bytes
  # Returns [klv, bytes_consumed]
  def self.parse(raw)
    # Split only on first two colons
    parts = raw.split(':', 3)
    raise "invalid KLV item: not enough pieces" if parts.length < 3
    
    key = parts[0].force_encoding('UTF-8')
    value_len = parts[1].to_i
    rest = parts[2]
    
    raise "not enough bytes for value" if rest.length < value_len
    value = rest[0...value_len]
    
    # Check for trailing newline
    if rest.length > value_len && rest[value_len] != "\n"
      raise "did not find \\n after value for key '#{key}'"
    end
    
    # Consume the value plus the trailing newline if present
    consumed = parts[0].length + 1 + parts[1].length + 1 + value_len
    consumed += 1 if rest.length > value_len  # Add 1 for the newline
    
    [KLV.new(key, value), consumed]
  end
end

# Benchmark configuration parsed from stdin
class Config
  attr_accessor :name, :model, :patterns, :case_insensitive, :unicode,
                :haystack, :max_iters, :max_warmup_iters, :max_time, :max_warmup_time

  def initialize
    @name = ''
    @model = ''
    @patterns = []
    @case_insensitive = false
    @unicode = false
    @haystack = ''
    @max_iters = 0
    @max_warmup_iters = 0
    @max_time = 0
    @max_warmup_time = 0
  end

  # Parse configuration from stdin in KLV format
  def self.parse
    config = Config.new
    raw = STDIN.binmode.read
    
    # Debug: if no input, show error
    if raw.nil? || raw.empty?
      STDERR.puts "No input received from stdin"
      exit 1
    end
    
    while raw.length > 0
      klv, nread = KLV.parse(raw)
      raw = raw[nread..-1]
      
      case klv.key
      when 'name'
        config.name = klv.value.force_encoding('UTF-8')
      when 'model'
        config.model = klv.value.force_encoding('UTF-8')
      when 'pattern'
        config.patterns << klv.value.force_encoding('UTF-8')
      when 'case-insensitive'
        config.case_insensitive = klv.value == 'true'.b
      when 'unicode'
        config.unicode = klv.value == 'true'.b
      when 'haystack'
        config.haystack = klv.value
      when 'max-iters'
        config.max_iters = klv.value.force_encoding('UTF-8').to_i
      when 'max-warmup-iters'
        config.max_warmup_iters = klv.value.force_encoding('UTF-8').to_i
      when 'max-time'
        config.max_time = klv.value.force_encoding('UTF-8').to_i
      when 'max-warmup-time'
        config.max_warmup_time = klv.value.force_encoding('UTF-8').to_i
      else
        raise "unrecognized KLV item key '#{klv.key}'"
      end
    end
    
    config
  end

  def get_haystack
    # In Ruby, we handle encoding based on whether Unicode mode is requested
    if @unicode
      @haystack.force_encoding('UTF-8')
    else
      @haystack.force_encoding('BINARY')
    end
  end

  def get_one_pattern
    raise "expected 1 pattern, but got #{@patterns.length}" if @patterns.length != 1
    pattern = @patterns[0]
    @unicode ? pattern.force_encoding('UTF-8') : pattern.force_encoding('BINARY')
  end
end

# Timer utilities
class Timer
  def self.run(config, &block)
    # Warmup phase
    warmup_start = Time.now.to_f
    config.max_warmup_iters.times do
      result = block.call
      break if (Time.now.to_f - warmup_start) * 1_000_000_000 >= config.max_warmup_time
    end
    
    # Actual benchmarking
    results = []
    run_start = Time.now.to_f
    config.max_iters.times do
      bench_start = Time.now.to_f
      count = block.call
      elapsed = ((Time.now.to_f - bench_start) * 1_000_000_000).to_i
      results << [elapsed, count]
      break if (Time.now.to_f - run_start) * 1_000_000_000 >= config.max_time
    end
    
    results
  end
end

# Model implementations
class Model
  def self.run(config, regex)
    case config.model
    when 'compile'
      run_compile(config)
    when 'count'
      run_count(config, regex)
    when 'count-spans'
      run_count_spans(config, regex)
    when 'count-captures'
      run_count_captures(config, regex)
    when 'grep'
      run_grep(config, regex)
    when 'grep-captures'
      run_grep_captures(config, regex)
    when 'regex-redux'
      run_regex_redux(config)
    else
      raise "unrecognized model '#{config.model}'"
    end
  end

  def self.run_compile(config)
    pattern = config.get_one_pattern
    options = config.case_insensitive ? Regexp::IGNORECASE : 0
    haystack = config.get_haystack
    
    Timer.run(config) do
      regex = Regexp.new(pattern, options)
      # For compile model, we still need to count matches in the haystack
      haystack.scan(regex).length
    end
  end

  def self.run_count(config, regex)
    haystack = config.get_haystack
    
    Timer.run(config) do
      haystack.scan(regex).length
    end
  end

  def self.run_count_spans(config, regex)
    haystack = config.get_haystack
    
    Timer.run(config) do
      sum = 0
      haystack.scan(regex) do
        match = Regexp.last_match
        sum += match.end(0) - match.begin(0)
      end
      sum
    end
  end

  def self.run_count_captures(config, regex)
    haystack = config.get_haystack
    
    Timer.run(config) do
      count = 0
      haystack.scan(regex) do
        match = Regexp.last_match
        # Count all capture groups including group 0
        (0...match.length).each do |i|
          count += 1 if match[i]
        end
      end
      count
    end
  end

  def self.run_grep(config, regex)
    haystack = config.get_haystack
    
    Timer.run(config) do
      count = 0
      haystack.each_line do |line|
        count += 1 if regex.match?(line)
      end
      count
    end
  end

  def self.run_grep_captures(config, regex)
    haystack = config.get_haystack
    
    Timer.run(config) do
      count = 0
      haystack.each_line do |line|
        line.scan(regex) do
          match = Regexp.last_match
          (0...match.length).each do |i|
            count += 1 if match[i]
          end
        end
      end
      count
    end
  end

  def self.run_regex_redux(config)
    # For now, we'll skip this complex model
    raise "regex-redux model not implemented"
  end
end

# Main execution
def main
  # Get engine type from command line
  if ARGV.empty?
    STDERR.puts "Usage: #{$0} <engine>"
    exit 1
  end
  
  engine = ARGV[0]
  
  # Handle version request
  if ARGV.include?('--version')
    puts RUBY_VERSION
    exit 0
  end
  
  # Ruby only has one regex engine
  unless engine == 'onigmo'
    STDERR.puts "unrecognized engine '#{engine}'"
    STDERR.puts "Ruby only supports 'onigmo' as the engine name"
    exit 1
  end
  
  # Parse configuration
  config = Config.parse
  
  # Compile regex if needed
  if config.model != 'compile'
    pattern = config.get_one_pattern
    options = config.case_insensitive ? Regexp::IGNORECASE : 0
    regex = Regexp.new(pattern, options)
  else
    regex = nil
  end
  
  # Run the model
  results = Model.run(config, regex)
  
  # Output results in CSV format
  results.each do |elapsed, count|
    puts "#{elapsed},#{count}"
  end
end

# Run if executed directly
main if __FILE__ == $0