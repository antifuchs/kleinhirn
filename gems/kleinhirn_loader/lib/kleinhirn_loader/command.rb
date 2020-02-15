# typed: strict
# frozen_string_literal: true

require 'kleinhirn_loader'

module KleinhirnLoader
  # The command language that is understood by the worker.
  class Command
    extend T::Sig
    extend T::Helpers

    abstract!
    sealed!

    sig do
      params(line: String)
        .returns(Command)
    end
    def self.parse(line)
      begin
        obj = JSON.parse(line)
        case (kind = Kind.deserialize(obj['op']))
        when Kind::Spawn
          unless obj.include?('id')
            return Error.new(line, RuntimeError.new('must include an "id" field'))
          end

          Spawn.new(obj['id'])
        else
          T.absurd(kind)
        end
      rescue JSON::ParserError => e
        Error.new(line, e)
      end
    end

    # The 'spawn' command.
    class Spawn < Command
      extend T::Sig
      extend T::Helpers

      sig do
        params(id: String)
          .void
      end
      def initialize(id)
        @id = id
      end

      sig { returns(String) }
      attr_reader :id
    end

    # An error reading a command. Not an actual command.
    class Error < Command
      sig do
        params(line: String, error: StandardError)
          .void
      end
      def initialize(line, error)
        @line = line
        @error = error
      end

      sig { returns(String) }
      attr_reader :line

      sig { returns(Exception) }
      attr_reader :error
    end

    # A deserialization helper enum
    class Kind < T::Enum
      enums do
        Spawn = new('spawn')
      end
    end
  end
end
