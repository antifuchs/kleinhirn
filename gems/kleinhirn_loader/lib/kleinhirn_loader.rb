# typed: strict
# frozen_string_literal: true

require 'sorbet-runtime'
require 'logger'

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
    log.info("Loading #{entry_point.inspect}...")
    load(entry_point)
    puts 'ready'
  end

  sig { void }
  def repl
    loop do
      case line = STDIN.gets&.chomp!
      when nil
        exit(0)
      when 'spawn'
        did = fork_one
        if did
          puts 'ok'
        else
          puts 'error'
        end
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

  # Clean up the worker process environment:
  #  * Close stdin
  #  * change working directory to `/`
  sig { void }
  def clean_child_environment
    STDIN.close
    Dir.chdir('/')
  end

  # Double-forks one pre-loaded worker process. The real PID is
  # discarded. If the intermediary child exited with a non-zero exit
  # status, returns `false`.
  sig { returns(T::Boolean) }
  def fork_one
    if (pid = Process.fork)
      # we're the initial parent - wait for the immediate child.
      until pid == (exited_pid = Process.waitpid(pid))
        log.warn("False intermediary #{exited_pid} exited with status #{$?.exitstatus}")
      end
      return $?.exitstatus.zero?
    end

    # This is the first sub-child. Unset most of our environment and
    # fork again:
    clean_child_environment
    exit(0) if Process.fork

    # Now we're in the worker - start it up.
    log.info("Worker #{Process.pid} running #{@expression.inspect}...")
    eval(@expression, Empty.new.to_binding) # rubocop:disable Security/Eval
    exit(0)
  end
end
