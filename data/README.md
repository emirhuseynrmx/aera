# Data Fixtures

This folder stores local demo inputs and reference data used by AeraCFO.

| File type | Purpose |
| --- | --- |
| `demo_*.csv` | Static SME finance scenarios used by `/api/demo`. |
| `incentives.json` | Reference data for the incentive-search tool. |

The demo CSVs are intended for local development, repeatable product demos, and smoke testing. They are not production customer data.

To regenerate demo scenarios:

```bash
cargo run --bin regenerate_demos
```

To inspect demo health-score spread:

```bash
cargo run --bin score_demos
```
