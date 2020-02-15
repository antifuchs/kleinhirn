# typed: strict
# frozen_string_literal: true

module KleinhirnLoader
  # Contains all the environment variables that the kleinhirn loader sets.
  class Env < T::Enum
    enums do
      # The ID that was assigned to the worker by the supervisor process.
      WorkerID = new('KLEINHIRN_WORKER_ID')

      # Name of the supervision group, for appropriate process naming
      Name = new('KLEINHIRN_NAME')

      # The status FD number.
      StatusFD = new('KLEINHIRN_STATUS_FD')
    end

    # Sets the corresponding environment variable
    sig do
      params(value: String)
        .void
    end
    def env=(value)
      ENV[to_s] = value
    end

    # Returns the corresponding environment variable
    sig do
      returns(T.nilable(String))
    end
    def env
      ENV[to_s]
    end

    # Unsets the environment variable.
    sig do
      void
    end
    def unset
      ENV.delete(to_s)
    end
  end
end
