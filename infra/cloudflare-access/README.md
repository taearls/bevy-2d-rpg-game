# Cloudflare Access allow-list (Terraform)

Infrastructure-as-code for the Cloudflare Access (Zero Trust) gate in front of
the wasm site. This recreates the dashboard setup described in the repo
[`README.md`](../../README.md) "Play in the browser" section: a self-hosted
Access application on the Pages hostname plus a single Allow policy whose
include list is the permitted emails.

Use it to stand up the allow-list for the renamed deploy (`aliasing.pages.dev`)
without clicking through the dashboard, and to manage the allow-list as code
thereafter.

## What it manages

- `cloudflare_zero_trust_access_policy.allow_emails` — a **reusable** Allow
  policy; its `include` is the email allow-list.
- `cloudflare_zero_trust_access_application.site` — the self-hosted app on
  `var.app_domain`, referencing the policy above.

**Not** managed here: the **One-time PIN** login method. That is account-level
(Zero Trust → Settings → Authentication → Login methods), not an app resource —
enable it once in the dashboard. With no identity provider configured, Access
uses One-time PIN automatically.

## Prerequisites

- [Terraform](https://developer.hashicorp.com/terraform) ≥ 1.6 (or OpenTofu).
- A Cloudflare API token exported as `CLOUDFLARE_API_TOKEN`, with:
  - **Access: Apps and Policies — Edit**
  - **Access: Organizations, Identity Providers, and Groups — Read**

  (This is a different, broader token than the `CLOUDFLARE_API_TOKEN` GitHub
  secret used by the deploy workflow, which only needs *Pages: Edit*. Use a
  separate token for Terraform.)
- Your Cloudflare **account ID** (same value as the `CLOUDFLARE_ACCOUNT_ID`
  deploy secret).

## Usage

```sh
cd infra/cloudflare-access

export CLOUDFLARE_API_TOKEN=...            # never commit this
cp terraform.tfvars.example terraform.tfvars   # then edit: account_id + allowed_emails

terraform init
terraform plan          # review — should create 1 policy + 1 application
terraform apply
```

### Managing the allow-list

Edit `allowed_emails` in `terraform.tfvars`, then `terraform apply`. Adding an
address shares access; removing one revokes it. This is the code equivalent of
editing the policy's Include → Emails list in the dashboard.

## Migrating an existing (dashboard-created) app

If the old `bevy-2d-rpg-game.pages.dev` app already exists and you want Terraform
to own the equivalent for the new hostname, you have two options:

1. **Fresh app for the new hostname (simplest).** Apply this module as-is to
   create a new app on `aliasing.pages.dev`. Leave the old app alone until the
   new URL is verified, then delete the old app in the dashboard (or import and
   `terraform destroy` it). Nothing about the old app is touched.

2. **Import the existing app.** If you'd rather Terraform adopt the current app,
   `terraform import` the application and policy by their IDs (find them in the
   dashboard URL or via the API) before `apply`, so Terraform reconciles instead
   of creating duplicates:

   ```sh
   terraform import cloudflare_zero_trust_access_application.site <account_id>/<app_id>
   terraform import cloudflare_zero_trust_access_policy.allow_emails <account_id>/<policy_id>
   ```

   Then update `app_domain` and re-apply. Review the plan carefully — the v5
   provider has had rough edges importing reusable-vs-inline policies, so
   confirm the plan is a no-op (or an intended change) before applying.

## State & secrets

`terraform.tfstate`, `terraform.tfvars`, and `.terraform/` are gitignored — state
holds the account ID and allow-list in plaintext. For a shared/CI setup, use a
remote backend (e.g. an R2/S3 bucket) instead of local state; none is configured
here so the module works standalone out of the box.
