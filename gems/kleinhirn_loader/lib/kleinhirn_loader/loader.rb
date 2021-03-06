# typed: strict
# frozen_string_literal: true

require 'kleinhirn_loader'
require 'kleinhirn_loader/env'
require 'kleinhirn_loader/command'
require 'kleinhirn_loader/replies'

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

      state_update(KleinhirnLoader::Replies::Loading.new(entry_point))
      load(entry_point)
      force_move_to_oldgen
    end

    # Runs the command loop for the kleinhirn worker. The command loop
    # protocol is JSONL (newline-separated json objects).
    sig { void }
    def repl
      process_name = "#{@name}/#{@version} ::KleinhirnLoader::Loader"
      Process.setproctitle(process_name)
      Process.setpgrp

      state_update(KleinhirnLoader::Replies::Ready.new)
      loop do
        unless (line = @status_io.gets&.chomp!)
          exit(0)
        end

        command = KleinhirnLoader::Command.parse(line)
        case command
        when KleinhirnLoader::Command::Spawn
          id = command.id
          if @worker_ids.include?(id)
            state_update(KleinhirnLoader::Replies::Failed.new(id, 'duplicate ID'))
          else
            fork_one(id)
            @worker_ids << id
          end
        when KleinhirnLoader::Command::Error
          state_update(KleinhirnLoader::Replies::Error.new('in command processing', command.error))
        else
          T.absurd(command)
        end
      end
    end

    private

    sig do
      params(reply: KleinhirnLoader::Replies::AbstractReply)
        .void
    end
    def state_update(reply)
      @status_io.puts(reply.to_json)
      @status_io.flush
    end

    sig do
      params(msg: String, fields: String).void
    end
    def log_info(msg, **fields)
      level = KleinhirnLoader::Replies::Log::Level::Info
      state_update(KleinhirnLoader::Replies::Log.new(level, msg, fields))
    end

    sig do
      params(msg: String, fields: String).void
    end
    def log_debug(msg, **fields)
      level = KleinhirnLoader::Replies::Log::Level::Debug
      state_update(KleinhirnLoader::Replies::Log.new(level, msg, fields))
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
      params(child_id: String)
        .void
    end
    def setup_child_environment(child_id)
      Dir.chdir('/')
      reseed_random

      KleinhirnLoader::Env::WorkerID.env = child_id
      KleinhirnLoader::Env::Name.env = @name
      KleinhirnLoader::Env::Version.env = @version
      KleinhirnLoader::Env::StatusFD.env = @status_io.fileno.to_s
    end

    # Updates the random seeds in ruby's various RNGs. This comes from
    # Einhorn, which has a good explanation in
    # https://github.com/stripe/einhorn/blob/8d90062ede64025e219949c6c63f061540d99f80/lib/einhorn/command.rb#L367-L395
    sig { void }
    def reseed_random
      # reseed Kernel#rand
      srand

      # reseed OpenSSL::Random if it's loaded
      if defined?(OpenSSL::Random)
        if (seed = defined?(Random))
          Random.new_seed
        else
          # Ruby 1.8
          seed = rand
        end
        OpenSSL::Random.seed(seed.to_s)
      end
    end

    # Make the GC more copy-on-write friendly by forcibly incrementing
    # the generation counter on all objects to its maximum
    # value. Learn more at: https://github.com/ko1/nakayoshi_fork
    # This comes from Einhorn, see the explanation in https://github.com/stripe/einhorn/blob/d1b79101eb9b111a2c2b1c7676f0310f54ab08de/lib/einhorn.rb#L258
    sig { void }
    def force_move_to_oldgen
      GC.start
      3.times do
        GC.start(full_mark: false)
      end

      T.unsafe(GC).compact if GC.respond_to?(:compact)
    end

    # Double-forks one pre-loaded worker process. The direct child's
    # PID is discarded, in expectation of getting re-parented to our
    # supervisor process.
    sig do
      params(child_id: String)
        .void
    end
    def fork_one(child_id)
      if (pid = Process.fork)
        # we're the initial parent - wait for the immediate child.
        until pid == Process.waitpid(pid); end
        state_update(KleinhirnLoader::Replies::Failed.new(child_id, 'non-zero exit')) unless $?.exitstatus.zero?
        return
      end

      # This is the first sub-child. Prepare our environment, fork
      # again, announce it and exit:
      setup_child_environment(child_id)
      if (pid = Process.fork)
        state_update(KleinhirnLoader::Replies::Launched.new(child_id, pid))
        exit(0)
      end

      # Now we're in the worker - start it up.
      process_name = "#{@name}/#{@version} ::KleinhirnLoader::Worker #{child_id} - startup"
      Process.setproctitle(process_name)
      log_info('worker starting', child_id: child_id, pid: Process.pid.to_s)
      eval(@expression, Empty.new.to_binding) # rubocop:disable Security/Eval
      exit(0)
    end
  end
end
