class User < ApplicationRecord
  atrr_accessor :name

  has_many :roles
end
