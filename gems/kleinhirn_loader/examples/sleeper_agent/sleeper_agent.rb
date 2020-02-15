#!/usr/bin/env ruby

# This file is a script that sleeps for 3 seconds, then exits.

require 'kleinhirn_loader/worker'

def kleinhirn_main
  sleep 2
  KleinhirnLoader::Worker.new.done

  puts Dir.pwd
  sleep 3
end

if $PROGRAM_NAME == __FILE__
  kleinhirn_main
end
