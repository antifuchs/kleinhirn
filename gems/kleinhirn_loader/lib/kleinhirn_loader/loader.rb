# typed: strict
# frozen_string_literal: true

require 'kleinhirn_loader'
require 'kleinhirn_loader/env'

require 'logger'
require 'set'

module KleinhirnLoader
  # Logic for the actual preloading "server" process
  class Loader
    extend T::Sig

    sig do
      params(name: String, version: String, expression: String, status_io: IO)
        .void
    end
    def initialize(name, version, expression, status_io)
      @log = T.let(initial_logger, Logger)
      @name = name
      @version = version
      @expression = expression
      @status_io = status_io
      @worker_ids = T.let(Set.new(), T::Set[String])
    end

    # Loads the input source file and, if successful, prints "ready".
    sig do
      params(entry_point: String)
        .void
    end
    def load_entrypoint(entry_point)
      log.info("Loading #{entry_point.inspect}...")
      load(entry_point)
      @status_io.puts 'ready'
    end

    sig { void }
    def repl
      loop do
        case line = @status_io.gets&.chomp!
        when nil
          exit(0)
        when /\Aspawn\s+(.+)\z/
          id = $1
          if @worker_ids.include?(id)
            log.error("Attempted to spawn second worker with ID #{id}")
            @status_io.puts("fail #{id}: duplicate worker ID")
          else
            fork_one(id)
            @worker_ids << id
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
    sig do
      params(child_id: String).
        void
    end
    def clean_child_environment(child_id)
      STDIN.close
      Dir.chdir('/')
      KleinhirnLoader::Env::WorkerID.env = child_id
      KleinhirnLoader::Env::Name.env = @name
      KleinhirnLoader::Env::Version.env = @version
      KleinhirnLoader::Env::StatusFD.env = @status_io.fileno.to_s
    end

    # Double-forks one pre-loaded worker process. The real PID is
    # discarded. If the intermediary child exited with a non-zero exit
    # status, returns `false`.
    sig do
      params(child_id: String).
        void
    end
    def fork_one(child_id)
      if (pid = Process.fork)
        # we're the initial parent - wait for the immediate child.
        until pid == (exited_pid = Process.waitpid(pid))
          log.warn("False intermediary #{exited_pid} exited with status #{$?.exitstatus}")
        end
        @status_io.puts "fail #{child_id}: non-zero exit" unless $?.exitstatus.zero?
        return
      end

      # This is the first sub-child. Unset most of our environment and
      # fork again:
      clean_child_environment(child_id)
      exit(0) if Process.fork

      # Now we're in the worker - start it up.
      process_name = "#{@name}/#{@version} ::KleinhirnLoader::Worker #{child_id} - startup"
      Process.setproctitle(process_name)
      log.info("Worker #{process_name} running #{@expression.inspect}...")
      eval(@expression, Empty.new.to_binding) # rubocop:disable Security/Eval
      exit(0)
    end
  end
end
