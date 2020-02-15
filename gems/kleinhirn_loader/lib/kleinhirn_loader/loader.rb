# typed: strict
# frozen_string_literal: true

require 'kleinhirn_loader'
require 'kleinhirn_loader/env'
require 'kleinhirn_loader/command'

require 'set'
require 'json'

module KleinhirnLoader
  # Logic for the actual preloading "server" process
  class Loader
    extend T::Sig

    sig do
      params(name: String, version: String, expression: String, status_io: IO)
        .void
    end
    def initialize(name, version, expression, status_io)
      @name = name
      @version = version
      @expression = expression
      @status_io = status_io
      @worker_ids = T.let(Set.new, T::Set[String])
    end

    # Loads the input source file and, if successful, prints "ready".
    sig do
      params(entry_point: String)
        .void
    end
    def load_entrypoint(entry_point)
      process_name = "#{@name}/#{@version} ::KleinhirnLoader::Loader loading #{entry_point}"
      Process.setproctitle(process_name)

      @status_io.puts "loading: #{entry_point}"
      load(entry_point)
    end

    # Runs the command loop for the kleinhirn worker. The command loop
    # protocol is JSONL (newline-separated json objects).
    sig { void }
    def repl
      process_name = "#{@name}/#{@version} ::KleinhirnLoader::Loader"
      Process.setproctitle(process_name)

      @status_io.puts "ready: #{@version}"
      loop do
        unless (line = @status_io.gets&.chomp!)
          exit(0)
        end

        command = KleinhirnLoader::Command.parse(line)
        case command
        when KleinhirnLoader::Command::Spawn
          id = command.id
          if @worker_ids.include?(id)
            @status_io.puts("fail #{id}: duplicate worker ID")
          else
            fork_one(id)
            @worker_ids << id
          end
        when KleinhirnLoader::Command::Error
          @status_io.puts("error: #{command.error.inspect}")
        else
          T.absurd(command)
        end
      end
    end

    private

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
      params(child_id: String)
        .void
    end
    def setup_child_environment(child_id)
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
      params(child_id: String)
        .void
    end
    def fork_one(child_id)
      if (pid = Process.fork)
        # we're the initial parent - wait for the immediate child.
        until pid == Process.waitpid(pid); end
        @status_io.puts "fail #{child_id}: non-zero exit" unless $?.exitstatus.zero?
        return
      end

      # This is the first sub-child. Prepare our environment, fork
      # again, announce it and exit:
      setup_child_environment(child_id)
      if (pid = Process.fork)
        @status_io.puts "launched #{child_id}: #{pid}"
        exit(0)
      end

      # Now we're in the worker - start it up.
      process_name = "#{@name}/#{@version} ::KleinhirnLoader::Worker #{child_id} - startup"
      Process.setproctitle(process_name)
      eval(@expression, Empty.new.to_binding) # rubocop:disable Security/Eval
      exit(0)
    end
  end
end
