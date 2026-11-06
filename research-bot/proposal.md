# RecoverLink synthetic prototype

Run locally with Python 3, no packages or network needed:

```sh
python3 recoverlink.py --html demo.html
python3 -m unittest -v
```

Open `demo.html` in a browser for the offline evidence-review report. CLI accepts an alternative JSON file with `--data`.

This is a working deterministic serial/fuzzy-text prototype, not a trained AI model or proof of production accuracy. It demonstrates why a lead was shortlisted, rejects contradictory serials, and refuses to treat same-model descriptions as verified matches. Source references point only to made-up records. All decisions remain unreviewed; no enforcement/contact actions are implemented.

A future AI version could extract model/serial text and rank messy descriptions/photos. Exact matching should remain deterministic. Validate extraction, candidate precision, missed matches, human review time and corrections before claiming savings. Do not infer a seller's criminal involvement.

Suggested recording: show exact serial lead; show same-model/conflicting-serial rejection; show shared marking candidate; explain human verification. No real seller data or case records.
