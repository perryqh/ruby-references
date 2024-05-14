class ClientInvitation < ApplicationRecord
  include HasUuid

  has_paper_trail
  belongs_to :accounting_firm, optional: true

  validates :accounting_firm, presence: true
  validates :name, presence: true
  validates :invited_by_user_id, presence: true
  validates :client_email, email: true, length: { maximum: 255 }, if: :client_email?
  validate :non_firm_email, if: :accounting_firm
  validates :uuid, uniqueness: { case_sensitive: true }

  class InvitationType < T::Enum
    enums do
      Unknown = new('unknown')
      EmailInvite = new('email_invite')
      InAppAdd = new('in_app_add')
      ProspectEmail = new('prospect_email')
    end
  end

  class InvitationTrigger < T::Enum
    enums do
      Immediate = new('Immediate')
      Onboarded = new('Onboarded')
    end
  end

  validates(
    :invitation_type,
    inclusion: InvitationType.values.map(&:serialize),
    allow_blank: true
  )

  private

  def non_firm_email
    if accounting_firm.accountants.map(&:email).include?(client_email)
      errors.add(:client_email, 'Email of firm member cannot be used for client email')
    end
  end
end
