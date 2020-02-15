# frozen_string_literal: true

Gem::Specification.new do |s|
  s.name        = 'kleinhirn_loader'
  s.version     = '0.1.0'
  s.licenses    = ['MIT', 'Apache-2.0']
  s.summary     = 'A very very minimal ruby code pre-loader for use with kleinhirn for process supervision.'
  s.authors     = ['Andreas Fuchs']
  s.email       = 'asf@boinkor.net'

  s.executables = Dir.glob('bin/**/*').map { |path| path.gsub('bin/', '') }
  s.files       = Dir.glob('lib/**/*')

  s.required_ruby_version = ['>= 2.6.0']

  s.add_dependency 'sorbet-runtime'
end
