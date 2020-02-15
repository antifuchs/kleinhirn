# typed: strict
# frozen_string_literal: true

require 'sorbet-runtime'

# KleinhirnLoader is the library that preloads a ruby project, keeps
# it running, and knows how to fork new worker processes.
module KleinhirnLoader
  DEFAULT_STATUS_FD = 3
end
