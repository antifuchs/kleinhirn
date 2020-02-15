# typed: strict
# frozen_string_literal: true

require 'optparse'
require 'kleinhirn_loader/loader'

module KleinhirnLoader
  class CLI
    extend T::Sig

    sig do
      params(args: T::Array[String]).
        void
    end
    def self.main(args)
      options = {
        status_fd: KleinhirnLoader::DEFAULT_STATUS_FD
      }
      load_files = []
      entrypoint = T.let(nil, T.nilable(String))
      op = OptionParser.new do |opts|
        opts.banner = "Usage: kleinhirn_loader [options] (-r load_file.rb ...) -e expression"

        opts.on('-r FILE', '--require=FILE', 'Preload this file. Can be given more than once') do |file|
          raise "#{file.inspect} not found" unless File.exist?(file)
          load_files << file
        end

        opts.on('-e EXPR', '--eval=EXPR', 'Run this expression in workers') do |expression|
          raise "Only one --eval argument can be used at a time." unless entrypoint.nil?
          entrypoint = expression
        end

        opts.on('--status-fd FD', 'Status FD to use for communicating with supervisor') do |status_fd|
          options[:status_fd] = status_fd.to_i
        end
      end
      op.parse!(args)

      if load_files.empty?
        puts "Need at least one -r file so load."
        puts op.help
        exit(1)
      end
      if entrypoint.nil?
        puts "Need one entry point."
        puts op.help
        exit(1)
      end

      status_fd = T.let(IO.new(options.fetch(:status_fd)), IO)
      kh = KleinhirnLoader::Loader.new(entrypoint, status_fd)
      load_files.each { |f| kh.load_entrypoint(f) }
      kh.repl
    end
  end
end
