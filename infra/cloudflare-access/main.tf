# Cloudflare Access (Zero Trust) allow-list for the wasm site.
#
# Recreates, as code, the setup documented in the repo README's "Play in the
# browser" section: a self-hosted Access application in front of the Pages
# deploy, with a single Allow policy whose include list is the permitted emails.
# Visitors not on the list are sent to Cloudflare's login and denied.
#
# One-time PIN login is an *account-level* login method (Zero Trust → Settings →
# Authentication), not an application resource, so it is intentionally NOT
# managed here — enable it once in the dashboard. With no identity provider
# configured, Access falls back to One-time PIN automatically.

# The allow-list policy. Kept as a standalone (reusable) policy rather than
# inlined on the application so the same allow-list can be attached to more than
# one app/hostname (e.g. during the rename, when both the old and new
# *.pages.dev hostnames may briefly coexist) and edited in one place.
resource "cloudflare_zero_trust_access_policy" "allow_emails" {
  account_id = var.account_id
  name       = "${var.app_name} allow-list"
  decision   = "allow"

  # OR-matched: a visitor passes if their email matches any entry.
  include = [
    for email in var.allowed_emails : {
      email = {
        email = email
      }
    }
  ]
}

# The self-hosted application protecting the Pages hostname.
resource "cloudflare_zero_trust_access_application" "site" {
  account_id       = var.account_id
  name             = var.app_name
  type             = "self_hosted"
  session_duration = var.session_duration

  # `destinations` supersedes the deprecated `self_hosted_domains`; a single
  # public hostname destination is the equivalent of the old `domain` field.
  destinations = [
    {
      type = "public"
      uri  = var.app_domain
    }
  ]

  # Attach the reusable allow-list policy. Only `id` + `precedence` belong here
  # for a reusable policy (decision/include live on the policy resource above).
  policies = [
    {
      id         = cloudflare_zero_trust_access_policy.allow_emails.id
      precedence = 1
    }
  ]
}
