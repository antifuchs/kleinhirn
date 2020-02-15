Gem::Specification.new do |s|
  s.name        = 'kleinhirn_loader'
  s.version     = '0.1.0'
  s.licenses    = ['MIT']
  s.summary     = 'A very very minimal ruby code pre-loader for use with kleinhirn for process supervision.'
  s.authors     = ['Andreas Fuchs']
  s.email       = 'asf@boinkor.net'

  s.files       = ['bin/kleinhirn_loader', 'lib/kleinhirn_loader.rb']

  s.add_runtime_dependency 'sorbet-runtime'
end
