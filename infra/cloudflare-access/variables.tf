variable "account_id" {
  description = "Cloudflare account ID that owns the Pages project and Access app (same value as the CLOUDFLARE_ACCOUNT_ID GitHub Actions secret used by the deploy workflow)."
  type        = string
}

variable "app_domain" {
  description = "The hostname Access protects. For the renamed Pages deploy this is aliasing.pages.dev."
  type        = string
  default     = "aliasing.pages.dev"
}

variable "app_name" {
  description = "Display name of the Access application in the Zero Trust dashboard."
  type        = string
  default     = "aliasing"
}

variable "allowed_emails" {
  description = "The allow-list: exact email addresses permitted through Access. Everyone not matched is denied. Add/remove an address here and re-apply to share/revoke access."
  type        = list(string)

  validation {
    condition     = length(var.allowed_emails) > 0
    error_message = "allowed_emails must contain at least one address, or the app would deny everyone."
  }
}

variable "session_duration" {
  description = "How long a successful login stays valid before re-authentication (Cloudflare duration string, e.g. \"24h\", \"30m\")."
  type        = string
  default     = "24h"
}
