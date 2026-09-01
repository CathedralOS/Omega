# Proof-checker bootstrap tools

| Retained file | Role | Deletion condition |
| --- | --- | --- |
| `artifact_env.sh` | Materializes the canonical checker tape. | Delete when Alpha accepts raw tapes directly. |
| `construct-artifact.sh` | Constructs a requested checker tape through the canonical Gamma compiler. | Delete only when a stronger exact construction replaces it. |
| `rebuild-artifact.sh` | Deliberately replaces the persisted checker tape. | Delete when artifact installation uses another equally explicit path. |
