#!/usr/bin/env ruby

# This file is a script that sleeps for 3 seconds, then exits.

def kleinhirn_main
  puts Dir.pwd
  sleep 3
end

if $PROGRAM_NAME == __FILE__
  kleinhirn_main
end
