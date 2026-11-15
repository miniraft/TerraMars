#!/usr/bin/env python3
"""Fetch modest regional MCD 6.2 climatology exports; never fabricate data.

Fifteen variables at each of two crater centers, 15-degree Ls / hourly LT grid.
Raw credited exports, query URLs and checksums are retained for auditability.
"""
import datetime,hashlib,json,re,time,urllib.parse,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BASE='https://www-mars.lmd.jussieu.fr/mcd_python/'
VARIABLES=['t','ps','u','v','tsurf','rho','tauref','fluxsurf_dn_sw','dust_dep','thermal_inertia','ground_albedo','col_h2ovapor','vmr_h2o','vmr_co2','pscaleheight']
def download(url):
 with urllib.request.urlopen(url,timeout=90) as r:return r.read()
def main():
 rawdir=ROOT/'assets/data/climate';rawdir.mkdir(exist_ok=True)
 result={'model':'Mars Climate Database 6.2','credit':'LMD/OU/IAA/ESA/CNES','source':BASE,'scenario':'climatology average solar','heightMeters':1.5,'retrievedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'notes':'Regional modeled climatology, not observations or a current storm forecast. Bilinear periodic interpolation of Ls and local true solar time; center coordinates only, no per-pixel terrain downscaling. Raw endpoint rows retained; duplicate 360 degree and 24 hour endpoints excluded from periodic interpolation.','sites':{}}
 for site,lat,lon in [('jezero',18.41,77.69),('gale',-5.44,137.70)]:
  grids={}
  for var in VARIABLES:
   path=rawdir/f'{site}_{var}.txt'; meta=path.with_suffix('.json')
   params=dict(var1=var,datekeyhtml=1,ls='all',localtime='all',latitude=lat,longitude=lon,altitude=1.5,zkey=3,dust=1,hrkey=1,averaging='off',isfixedlt='off',animation='off',dpi=80,islog='off',colorm='jet',iswind='off')
   query=BASE+'cgi-bin/mcdcgi.py?'+urllib.parse.urlencode(params)
   if not path.exists():
    page=download(query).decode();m=re.search(r"href=['\"]([^'\"]+\.txt)['\"]",page)
    if not m:raise RuntimeError(f'No ASCII export for {site}/{var}: {page[-600:]}')
    url=urllib.parse.urljoin(BASE+'cgi-bin/',m[1]);body=download(url);path.write_bytes(body)
    meta.write_text(json.dumps({'query':query,'asciiExport':url,'sha256':hashlib.sha256(body).hexdigest()},indent=2)+'\n');time.sleep(.25)
   text=path.read_text();rows=[]
   for line in text.splitlines():
    if re.match(r'^\+\d+\s*\|\|',line):rows.append([float(n) for n in line.split('||')[1].split()])
   if len(rows)!=25 or any(len(row)!=25 for row in rows):raise RuntimeError(f'Unexpected grid {site}/{var}')
   if not all(-1e20<n<1e20 for row in rows for n in row):raise RuntimeError(f'Missing values in grid {site}/{var}')
   grids[var]={'values':[row[:24] for row in rows[:24]],'raw':str(path.relative_to(ROOT)),**json.loads(meta.read_text())}
   print(site,var,'verified 25 x 25 source samples',flush=True)
  result['sites'][site]={'latitude':lat,'longitude':lon,'grids':grids}
  (ROOT/'assets/data/climate_model.json').write_text(json.dumps(result,separators=(',',':'))+'\n')
if __name__=='__main__':main()
