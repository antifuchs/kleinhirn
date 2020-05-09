# typed: strict
# frozen_string_literal: true

module KleinhirnLoader
  # Contains all the environment variables that the kleinhirn loader sets.
  class Env < T::Enum
    extend T::Sig

    enums do
      # The ID that was assigned to the worker by the supervisor process.
      WorkerID = new('KLEINHIRN_WORKER_ID')

      # Name of the supervision group, for appropriate process naming.
      Name = new('KLEINHIRN_NAME')

      # Version of the code that this supervision process group has loaded.
      Version = new('KLEINHIRN_VERSION')

      # The status FD number.
      StatusFD = new('KLEINHIRN_STATUS_FD')
    end

    # Sets the corresponding environment variable
    sig do
      params(value: String)
        .void
    end
    def env=(value)
      ENV[serialize] = value
    end

    # Returns the corresponding environment variable
    sig do
      returns(T.nilable(String))
    end
    def env
      ENV[serialize]
    end

    # Unsets the environment variable.
    sig do
      void
    end
    def unset
      ENV.delete(serialize)
    end
  end
end
