module HasUuid
  extend ActiveSupport::Concern
  included do
    before_validation :generate_uuid
    before_save :generate_uuid

    validates :uuid, presence: true, if: :uuid_backfilled?
    validates :uuid, presence: { allow_nil: true }, unless: :uuid_backfilled?
  end

  private

  def generate_uuid
    self.uuid ||= new_uuid
  end

  def new_uuid
    SecureRandom.uuid
  end
end
