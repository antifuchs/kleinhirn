#!/usr/bin/env ruby
# frozen_string_literal: true

# This file is a script that waits 2 seconds, acks that it launched,
# sleeps for 3 seconds, then exits.

require 'kleinhirn_loader/worker'

def kleinhirn_main
  sleep 1
  KleinhirnLoader::Worker.new.done
  sleep 6
end

if $PROGRAM_NAME == __FILE__
  kleinhirn_main
end
