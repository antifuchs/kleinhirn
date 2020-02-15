# typed: strict
# frozen_string_literal: true

module KleinhirnLoader
  module Replies
    # A reply sent back to the supervisor.
    class AbstractReply
      extend T::Sig
      extend T::Helpers

      abstract!
      sealed!

      sig do
        abstract
          .params(args: T.untyped)
          .returns(String)
      end
      def to_json(*args); end
    end

    # Indicates that the loader is loading the given ruby file.
    class Loading < AbstractReply
      sig do
        params(file: String)
          .void
      end
      def initialize(file)
        @file = file
      end

      sig { override.params(_args: T.untyped).returns(String) }
      def to_json(*_args)
        {
          'action': 'loading',
          'file': @file,
        }.to_json
      end
    end

    # Indicates that all code has been properly loaded.
    class Ready < AbstractReply
      sig { override.params(_args: T.untyped).returns(String) }
      def to_json(*_args)
        {
          'action': 'ready',
        }.to_json
      end
    end

    # Indicates an input error, with details on the `error` and
    # `message` fields.
    class Error < AbstractReply
      sig do
        params(message: String, error: T.nilable(Exception))
          .void
      end
      def initialize(message, error)
        @message = message
        @error = error
      end

      sig { override.params(_args: T.untyped).returns(String) }
      def to_json(*_args)
        val = {
          'action': 'error',
          'message': @message,
        }
        if @error
          val['error'] = @error
        end
        val.to_json
      end
    end

    # Indicates an operation that has failed on the side of the loader.
    class Failed < AbstractReply
      sig do
        params(id: String, message: String)
          .void
      end
      def initialize(id, message)
        @id = id
        @message = message
      end

      sig { override.params(_args: T.untyped).returns(String) }
      def to_json(*_args)
        val = {
          'action': 'failed',
          'message': @message,
          'id': @id,
        }.to_json
      end
    end

    # Indicates that a worker process has been launched and is now
    # running its start ruby expression.
    class Launched < AbstractReply
      sig do
        params(id: String, pid: Integer)
          .void
      end
      def initialize(id, pid)
        @id = id
        @pid = pid
      end

      sig { override.params(_args: T.untyped).returns(String) }
      def to_json(*_args)
        val = {
          'action': 'launched',
          'id': @id,
          'pid': @pid,
        }.to_json
      end
    end

    # A worker process acknowledging to the supervisor that it is done
    # initializing & ready to serve requests.
    class Ack < AbstractReply
      sig do
        params(id: String)
          .void
      end
      def initialize(id)
        @id = id
      end

      sig { override.params(_args: T.untyped).returns(String) }
      def to_json(*_args)
        val = {
          'action': 'ack',
          'id': @id,
        }.to_json
      end
    end
  end
end
