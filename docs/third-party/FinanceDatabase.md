# FinanceDatabase

The Search page's **Download symbol metadata** button (Company data section)
loads sector, industry, country and identifier data (ISIN, CUSIP, FIGI) for
US-listed equities and ETFs from **FinanceDatabase** by Jeroen Bouma, into the
app's DuckDB `symbol_metadata` table.

- Repository: https://github.com/JerBouma/FinanceDatabase
- Pinned commit: `1ac8809da480` (data as of 2026-11-08). Every download reads
  this exact commit, never the moving default branch.
- Files downloaded (`https://raw.githubusercontent.com/JerBouma/FinanceDatabase/1ac8809da480/…`):
  - `database/equities/{NYQ,NMS,NGM,NCM,ASE,PCX,BTS,OQB}.csv`
  - `database/etfs/{PCX,BTS,NGM,NMS,NYQ,NCM}.csv`
  - `database/equities/PNK.csv` (OTC pink sheets, 14.5 MB) only when
    **Include OTC (pink sheets)** is checked.
- Exchange codes are mapped to the app's exchange names: NYQ → NYSE,
  NMS/NGM/NCM → NASDAQ, ASE → AMEX, PCX → ARCA, BTS → BATS, OQB/PNK → OTC.
  Rows on any other exchange are skipped.
- `summary` is stored truncated to 2,000 characters; delisted rows are loaded
  with `delisted = true` and left out of search results by default.
- The app credits the source under the Company data table:
  "Data: FinanceDatabase (MIT)".
- Tests use small synthetic CSV rows shaped on the real headers, which were
  verified against `database/equities/NYQ.csv` and `database/etfs/PCX.csv` at
  the pinned commit on 2026-11-12. No test touches the network.

Implementation: `src-tauri/src/services/symbol_metadata_service.rs` (download,
parse, load), `src-tauri/src/db/duckdb/symbol_metadata.rs` (storage, search),
`src-tauri/src/commands/symbol_metadata.rs` (`symbol_metadata_*` commands),
`src/components/search/CompanyDataCard.tsx` (UI).

## License

FinanceDatabase is distributed under the MIT License. Text copied from
https://raw.githubusercontent.com/JerBouma/FinanceDatabase/1ac8809da480/LICENSE:

```text
MIT License

Copyright (c) 2023 Jeroen Bouma

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```
