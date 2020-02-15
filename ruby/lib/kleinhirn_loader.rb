# typed: strict
# frozen_string_literal: true

require 'sorbet-runtime'
require 'optparse'

# KleinhirnLoader is the library that preloads a ruby project, keeps
# it running, and knows how to fork new worker processes.
#
# It reads commands from stdin (as JSON) and
class KleinhirnLoader
  extend T::Sig

  sig do
    params(expression: String)
      .void
  end
  def initialize(expression)
    @log = T.let(initial_logger, Logger)
    @expression = expression
  end

  # Loads the input source file and, if successful, prints "ready".
  sig do
    params(entry_point: String)
      .void
  end
  def load_entrypoint(entry_point)
    load(entry_point)
    puts 'ready'
  end

  sig { void }
  def repl
    loop do
      case line = STDIN.gets
      when nil
        exit(0)
      when 'spawn'
        fork_one
      else
        log.error("Invalid command #{line.inspect} received, ignoring.")
      end
    end
  end

  private

  sig { returns(Logger) }
  attr_reader :log

  sig do
    returns(Logger)
  end
  def initial_logger
    logger = Logger.new(STDERR)
    logger.progname = 'kleinhirn_loader'
    logger
  end

  # An empty (except for sorbet type annotations) binding
  class Empty
    extend T::Sig
    sig { returns(Binding) }
    def to_binding
      self.class.remove_method(:to_binding)
      binding
    end
  end

  # Double-forks one pre-loaded worker process. The real PID is
  # discarded.
  sig { void }
  def fork_one
    return if Process.fork

    exit(0) if Process.fork

    # Now we're in the actual worker. Daemonize:
    STDIN.close
    Dir.chdir('/')

    # aaand run:
    log.info("Running #{@expression.inspect}...")
    eval(@expression, Empty.new.to_binding) # rubocop:disable Security/Eval
    exit(0)
  end
end
