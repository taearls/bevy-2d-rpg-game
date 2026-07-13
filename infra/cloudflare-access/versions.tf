terraform {
  required_version = ">= 1.6"

  required_providers {
    cloudflare = {
      source = "cloudflare/cloudflare"
      # v5 rewrote the Zero Trust Access resources (list-of-objects `include`,
      # `policies` on the application, `zero_trust_*` names). Pin to v5.x so the
      # schema below stays valid.
      version = "~> 5.0"
    }
  }
}

provider "cloudflare" {
  # The API token is read from the CLOUDFLARE_API_TOKEN environment variable —
  # never hard-code it here. The token needs the "Access: Apps and Policies:
  # Edit" and "Access: Organizations, Identity Providers, and Groups: Read"
  # permissions on the account. See README.md.
}
