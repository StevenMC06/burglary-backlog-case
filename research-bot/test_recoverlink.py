"""Illustrative case assumptions, not measured performance or vendor prices."""
import csv
from pathlib import Path
OUT=Path(__file__).resolve().parent
VOLUME=10_000; BASELINE=3_050_000; TARGET=457_500
BUILD=180_000
SCENARIOS=[
 dict(scenario='Conservative',coverage=.60,adoption=.70,net_minutes=40,reserve=.20,recurring=195_500,ramp=.50),
 dict(scenario='Base',coverage=.80,adoption=.85,net_minutes=60,reserve=.20,recurring=165_500,ramp=.65),
 dict(scenario='Optimistic',coverage=.90,adoption=.95,net_minutes=90,reserve=.10,recurring=145_500,ramp=.75),
]
rows=[]
for s in SCENARIOS:
 cases=VOLUME*s['coverage']*s['adoption']
 hours=cases*s['net_minutes']/60
 gross=hours*50
 release=gross*(1-s['reserve'])
 steady=release-s['recurring']
 year1=release*s['ramp']-s['recurring']-BUILD
 # Benefits exclude reserved officer time. Costs include recurring follow-up labor.
 roi=(release*s['ramp']-s['recurring']-BUILD)/(s['recurring']+BUILD)
 # End-year-one unrecovered amount, followed by constant full-adoption annual net benefit.
 payback=1+(-year1)/steady if year1<0 and steady>0 else None
 required_minutes=(TARGET+s['recurring'])/(VOLUME*s['coverage']*s['adoption']*50*(1-s['reserve']))*60
 rows.append(dict(scenario=s['scenario'],eligible_fraction=s['coverage'],adoption_fraction=s['adoption'],treated_cases=cases,net_minutes_saved_per_treated_case=s['net_minutes'],gross_officer_hours=hours,reserved_officer_hours=hours*s['reserve'],gross_labor_value=gross,releasable_labor_value=release,annual_recurring_cost=s['recurring'],one_time_cost=BUILD,first_year_ramp=s['ramp'],steady_net_economic_benefit=steady,steady_net_percent=steady/BASELINE,first_year_net_economic_benefit=year1,first_year_net_percent=year1/BASELINE,first_year_ROI=roi,payback_years_if_after_year1=payback,net_minutes_required_for_steady_target=required_minutes))
 assert abs(steady-(hours-hours*s['reserve'])*50+s['recurring'])<1e-7
 assert s['net_minutes']<=120
with (OUT/'scenarios.csv').open('w',newline='') as f:
 w=csv.DictWriter(f,fieldnames=rows[0]);w.writeheader();w.writerows(rows)
for r in rows:
 print(r['scenario'], {k:round(r[k],2) if isinstance(r[k],float) else r[k] for k in ['treated_cases','gross_labor_value','reserved_officer_hours','steady_net_economic_benefit','first_year_net_economic_benefit','net_minutes_required_for_steady_target']})
assert TARGET/BASELINE==.15
assert 10_000*.13*1.25-1300==325
print('Arithmetic checks passed. CSV regenerated.')
