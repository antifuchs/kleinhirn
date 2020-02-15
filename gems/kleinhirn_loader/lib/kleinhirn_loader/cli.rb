# typed: strict
# frozen_string_literal: true

require 'optparse'
require 'securerandom'
require 'kleinhirn_loader/loader'

module KleinhirnLoader
  # The command-line interface for kleinhirn_loader.
  class CLI
    extend T::Sig

    sig do
      params(args: T::Array[String])
        .void
    end
    def self.main(args)
      options = {
        status_fd: KleinhirnLoader::DEFAULT_STATUS_FD,
        name: File.basename(Dir.pwd),
        version: SecureRandom.uuid
      }
      load_files = []
      entrypoint = T.let(nil, T.nilable(String))
      op = OptionParser.new do |opts|
        opts.banner = 'Usage: kleinhirn_loader [options] (-r load_file.rb ...) -e expression'

        opts.on('-r FILE', '--require=FILE', 'Preload this file. Can be given more than once') do |file|
          raise "#{file.inspect} not found" unless File.exist?(file)

          load_files << file
        end

        opts.on('-e EXPR', '--eval=EXPR', 'Run this expression in workers') do |expression|
          raise 'Only one --eval argument can be used at a time.' unless entrypoint.nil?

          entrypoint = expression
        end

        opts.on('-n NAME', '--name=NAME', 'Name that should be assigned to workers') do |name|
          options[:name] = name
        end

        opts.on('--status-fd=FD', 'Status FD to use for communicating with supervisor') do |status_fd|
          options[:status_fd] = status_fd.to_i
        end

        opts.on('--code-version=VERSION',
                'String that identifies the version of code. Defaults to a random UUID.') do |version|
          options[:version] = version
        end
      end
      op.parse!(args)

      if load_files.empty?
        puts 'Need at least one -r file so load.'
        puts op.help
        exit(1)
      end
      if entrypoint.nil?
        puts 'Need one entry point.'
        puts op.help
        exit(1)
      end

      status_fd = T.let(IO.new(options.fetch(:status_fd)), IO)
      kh = KleinhirnLoader::Loader.new(options[:name], options[:version],
                                       entrypoint, status_fd)
      load_files.each { |f| kh.load_entrypoint(f) }
      kh.repl
    end
  end
end
