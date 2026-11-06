# Option B: RecoverLink — source-linked stolen-property matching

## TLDR
Compare victim-submitted stolen-item descriptions, photos, receipts, and serial numbers against authorized pawn/secondhand inventory feeds and lawfully available resale listings. Exact serial matching first; AI normalizes inconsistent brands/models, extracts visible text, and ranks photo/description candidates where exact identifiers are absent. Detective verifies ownership and original records before contacting anyone or seeking any hold. Seller is not automatically the burglar; match is an investigative lead, not probable cause by itself.

## Strong demo
Use 10 fictional stolen-item records and 100 synthetic resale listings. Show an exact serial match; a distinctive-marking/photo candidate; an identical model that is rejected for different serial; an uncertain candidate left unresolved. Show why each match was ranked and source links. Use deterministic exact match where available; AI must add value on messy descriptions/OCR, not merely rename a database lookup.

## Primary sources
1. Albuquerque Police official description of existing LeadsOnline workflow: businesses upload inventory/pictures; victim inventory includes serial numbers, descriptions, photos, receipts. https://www.cabq.gov/police/online-services/leads-online
2. Seattle Police published manual 15.210, effective 2012: documented stolen-property investigation uses model/serial numbers, unique markings, photos and receipts, with detective follow-up. Useful local workflow precedent, but do not imply all dated policy details are current law. https://public.powerdms.com/Sea4550/documents/2042780
3. Washington Legislature RCW 19.60.045: law-enforcement notification/holds require defined human process and follow-through. Do not automate legal holds based on fuzzy similarity. https://app.leg.wa.gov/Rcw/default.aspx?cite=19.60.045

## Feasibility and risk
- Start with a contracted/authorized pawn feed or synthetic demonstration. Do not promise unrestricted Facebook Marketplace, OfferUp, eBay, or LeadsOnline APIs or scrape behind login/against permission.
- Missing serials, generic products, altered identifiers, duplicate listings, unavailable feeds, and legitimate resale create false positives.
- Ownership proof and receipt/serial corroboration matter. No civilian seller confrontation, automated accusation, automated seizure, or automated hold.
- Limit seller personal data, access and retention; display provenance, confidence class, and human adjudication.
- Existing property-recovery products mean differentiation must be workflow integration, source verification, and measurable analyst time, not claiming matching is new.

## Economics vs video
Video has a fictional supplied 120-minute/$100-per-case allocation; property recovery has NO separate stage-7 baseline in the case. Never invent a stage-7 budget or add savings beyond $305/case without explicit new assumption/scope. Could test reduced detective lookup time within the existing 90 minutes; actual cost savings must be measured and offset by new follow-up work. Recovery is not burglary clearance: track recovered items/value separately from formal additional cleared burglary cases.

## Recommendation
Offer this as a strong distinct alternative with community benefit and a more tractable prototype, but video remains stronger on baseline-backed savings arithmetic. Property matching may be a better choice if the team prioritizes demo quality and victim property return and accepts that the 15% savings target is harder to support. Neither has proven causal clearance uplift. Human team should choose after reviewing both.


## Team discussion status
Two-option TLDR posted in general: https://discord.com/channels/1568105212017639466/1568105212839985324/1568117649999667285
No decision recorded yet. A human must explicitly support a specific approach. Earlier video recommendation and deck are analysis drafts, not team-approved choices.
