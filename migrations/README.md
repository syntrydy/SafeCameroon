# Database migrations

Migrations use SQLx's reversible naming convention: `NNNN_description.up.sql`
and `NNNN_description.down.sql`.

Production deployments run forward migrations only. Down migrations are for local
development and tested recovery procedures; production rollback uses a new,
forward corrective migration unless an approved recovery runbook says otherwise.

The first migration deliberately does not create organization or user tables.
Those will arrive with the authorization foundation, rather than weakening this
reporting slice with placeholders for access control.
