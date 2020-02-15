# typed: strict
# frozen_string_literal: true

require 'kleinhirn_loader'
require 'kleinhirn_loader/env'

module KleinhirnLoader
  # Worker processes - confirmation that startup is finished and
  # cleanup.
  class Worker
    extend T::Sig

    # Confirms to the supervisor that startup / initialization is done.
    sig do
      void
    end
    def done
      if confirm_loaded
        cleanup!
      end
    end

    private
    sig do
      returns(T::Boolean)
    end
    def confirm_loaded
      fd = KleinhirnLoader::Env::StatusFD.env&.to_i
      worker_id = KleinhirnLoader::Env::WorkerID.env
      name = KleinhirnLoader::Env::Name.env
      return false if fd.nil? || worker_id.nil?

      status_io = IO.new(fd)
      status_io.puts("ok #{worker_id}")
      status_io.close

      process_name = "#{name} ::KleinhirnLoader::Worker #{worker_id}"
      Process.setproctitle(process_name)
      true
    end

    # Removes any environment variables from the process that were set
    # by the kleinhirn preloader.
    sig do
      void
    end
    def cleanup!
      KleinhirnLoader::Env.values.each(&:unset)
    end
  end
end
