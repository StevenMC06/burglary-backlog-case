# Public geography source

`seattle_burglary_points.csv` is a deliberately thin extract of public Seattle burglary report locations. It supplies realistic *places* for the synthetic Cascade City (fictional) dataset. No incident detail, dates, victims or suspects are kept.

- **Dataset:** City of Seattle Open Data, "SPD Crime Data: 2008-Present" (Socrata id `tazs-3rd5`), published by the Seattle Police Department under the City of Seattle open data terms.
- **Endpoint:** `https://data.seattle.gov/resource/tazs-3rd5.json`
- **Query (SoQL):**
  - `$select=block_address,latitude,longitude,beat,neighborhood,offense_date`
  - `$where=offense_sub_category='BURGLARY' AND offense_date>='2024-11-05T00:00:00' AND latitude IS NOT NULL AND longitude IS NOT NULL`
  - `$order=offense_date DESC`
  - `$limit=50000`
- **Fetched:** 2026-11-05 (the API returned 14,935 rows; latest offense date in the dataset was 2026-11-04).
- **Cleaning:** dropped rows with missing, zero, redacted or out-of-city coordinates and rows without a beat (14,360 remain); then kept every 3rd row (deterministic thinning) to stay under 6,000 rows.
- **Result:** 4,787 rows. Columns: `block_address` (hundred-block), `lat`, `lon` (rounded to 5 decimals, the block-level coordinates published by SPD), `beat`, `neighborhood` (SPD MCPP name; `UNKNOWN` where SPD publishes "-").
- **Privacy:** public data at hundred-block granularity. No personal data. The `offense_date` column was used only for filtering and is not stored.
- **Use in Casefile:** `casefile datagen` samples these points and jitters them by up to 60 m. Generated cases are synthetic and are not Seattle incidents.
