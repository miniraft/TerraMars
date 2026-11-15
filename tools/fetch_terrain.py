#!/usr/bin/env python3
"""Extract real MOLA MEGDR 128ppd regional topography via bounded HTTP ranges."""
import hashlib,json,math,struct,urllib.request
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
BASE='https://pds-geosciences.wustl.edu/mgs/mgs-m-mola-5-megdr-l3-v1/mgsl_300x/meg128/'

def fetch():
 manifest=json.loads((ROOT/'assets/maps/manifest.json').read_text());out=ROOT/'assets/data/terrain';out.mkdir(parents=True,exist_ok=True)
 for site,tile,north,west in [('jezero','megt44n000hb',44,0),('gale','megt00n090hb',0,90)]:
  m=manifest[site];row0=math.floor((north-m['north'])*128)-2;row1=math.ceil((north-m['south'])*128)+2
  col0=math.floor((m['west']-west)*128)-2;col1=math.ceil((m['east']-west)*128)+2
  start=row0*23040;end=row1*23040-1;url=BASE+tile+'.img'
  cache=Path('/tmp')/f'terramars-{tile}-{row0}-{row1}.bin'
  if not cache.exists():
   request=urllib.request.Request(url,headers={'Range':f'bytes={start}-{end}','User-Agent':'TerraMars research viewer'})
   with urllib.request.urlopen(request,timeout=90) as response:
    if response.status!=206:raise RuntimeError('Server did not honor byte range')
    if not response.headers.get('Content-Range','').startswith(f'bytes {start}-{end}/'):raise RuntimeError('Wrong Content-Range')
    raw=response.read(end-start+2)
   if len(raw)!=end-start+1:raise RuntimeError('Truncated elevation range')
   cache.write_bytes(raw)
  raw=cache.read_bytes();crop=b''.join(raw[(r-row0)*23040+col0*2:(r-row0)*23040+col1*2] for r in range(row0,row1))
  (out/f'{site}.i16be').write_bytes(crop)
  label=urllib.request.urlopen(BASE+tile+'.lbl',timeout=30).read();(out/f'{tile}.lbl').write_bytes(label)
  metadata={'site':site,'product':'MGS MOLA MEGDR 128 pixels per degree topography','source_url':'https://pds-geosciences.wustl.edu/missions/mgs/megdr.html','download_url':url,'label_url':BASE+tile+'.lbl','source_range':f'bytes={start}-{end}','range_sha256':hashlib.sha256(raw).hexdigest(),'sha256':hashlib.sha256(crop).hexdigest(),'format':'big-endian signed 16-bit topography, metres relative to Mars areoid','columns':col1-col0,'rows':row1-row0,'west':west+col0/128,'north':north-row0/128,'degrees_per_cell':1/128,'nominal_resolution_m':463,'observed':'1997-09-15 through 2001-06-30','product_created':'2003-03-21','slope_limit_degrees':15,'policy_note':'Application planning exclusion threshold, not a NASA traverse limit or hazard certification.'}
  (out/f'{site}.json').write_text(json.dumps(metadata,indent=2)+'\n');print(site,metadata['columns'],metadata['rows'],len(crop),'bytes',flush=True)
if __name__=='__main__':fetch()
