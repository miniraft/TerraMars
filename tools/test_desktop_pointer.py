#!/usr/bin/env python3
"""Real Linux/X11 mouse/keyboard regression checks against the release app.

Uses XTest events, not calls into application handlers. No mission history is
modified: --interaction-test opens isolated temporary storage. Requires DISPLAY,
libX11 and libXtst. Screenshots are optional when Pillow is installed.
"""
import argparse,ctypes as c,json,os,subprocess,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
class Desktop:
 def __init__(self,pid):
  self.pid=pid
  self.x=c.CDLL('libX11.so.6');self.xt=c.CDLL('libXtst.so.6')
  self.x.XOpenDisplay.argtypes=[c.c_char_p];self.x.XOpenDisplay.restype=c.c_void_p
  self.d=self.x.XOpenDisplay(None)
  if not self.d:raise RuntimeError('An X11 DISPLAY is required')
  self.x.XDefaultRootWindow.argtypes=[c.c_void_p];self.x.XDefaultRootWindow.restype=c.c_ulong;self.root=self.x.XDefaultRootWindow(self.d)
  self.handler=c.CFUNCTYPE(c.c_int,c.c_void_p,c.c_void_p)(lambda *_:0)
  self.x.XSetErrorHandler.argtypes=[c.c_void_p];self.x.XSetErrorHandler(c.cast(self.handler,c.c_void_p))
  self.x.XInternAtom.argtypes=[c.c_void_p,c.c_char_p,c.c_int];self.x.XInternAtom.restype=c.c_ulong
  self.pid_atom=self.x.XInternAtom(self.d,b'_NET_WM_PID',0)
  self.x.XGetWindowProperty.argtypes=[c.c_void_p,c.c_ulong,c.c_ulong,c.c_long,c.c_long,c.c_int,c.c_ulong,c.POINTER(c.c_ulong),c.POINTER(c.c_int),c.POINTER(c.c_ulong),c.POINTER(c.c_ulong),c.POINTER(c.POINTER(c.c_ubyte))]
  self.x.XFetchName.argtypes=[c.c_void_p,c.c_ulong,c.POINTER(c.c_char_p)]
  self.x.XQueryTree.argtypes=[c.c_void_p,c.c_ulong,c.POINTER(c.c_ulong),c.POINTER(c.c_ulong),c.POINTER(c.POINTER(c.c_ulong)),c.POINTER(c.c_uint)]
  self.x.XFree.argtypes=[c.c_void_p];self.x.XFlush.argtypes=[c.c_void_p]
  self.x.XTranslateCoordinates.argtypes=[c.c_void_p,c.c_ulong,c.c_ulong,c.c_int,c.c_int,c.POINTER(c.c_int),c.POINTER(c.c_int),c.POINTER(c.c_ulong)]
  self.x.XRaiseWindow.argtypes=[c.c_void_p,c.c_ulong]
  self.x.XSync.argtypes=[c.c_void_p,c.c_int]
  self.x.XSetInputFocus.argtypes=[c.c_void_p,c.c_ulong,c.c_int,c.c_ulong]
  self.x.XKeysymToKeycode.argtypes=[c.c_void_p,c.c_ulong];self.x.XKeysymToKeycode.restype=c.c_uint
  self.xt.XTestFakeMotionEvent.argtypes=[c.c_void_p,c.c_int,c.c_int,c.c_int,c.c_ulong]
  self.xt.XTestFakeButtonEvent.argtypes=[c.c_void_p,c.c_uint,c.c_int,c.c_ulong]
  self.xt.XTestFakeKeyEvent.argtypes=[c.c_void_p,c.c_uint,c.c_int,c.c_ulong]
 def find(self,w=None):
  if w is None:w=self.root
  name=c.c_char_p()
  if self.x.XFetchName(self.d,w,c.byref(name)) and name.value:
   text=name.value.decode('utf8','replace');self.x.XFree(name)
   if text=='TerraMars Pointer Test':
    actual=c.c_ulong();fmt=c.c_int();n=c.c_ulong();left=c.c_ulong();data=c.POINTER(c.c_ubyte)()
    self.x.XGetWindowProperty(self.d,w,self.pid_atom,0,1,0,0,c.byref(actual),c.byref(fmt),c.byref(n),c.byref(left),c.byref(data))
    pid=c.cast(data,c.POINTER(c.c_ulong))[0] if data and n.value else 0
    if data:self.x.XFree(data)
    if pid==self.pid:return w
  rr=c.c_ulong();parent=c.c_ulong();kids=c.POINTER(c.c_ulong)();n=c.c_uint()
  if self.x.XQueryTree(self.d,w,c.byref(rr),c.byref(parent),c.byref(kids),c.byref(n)):
   ids=[kids[i] for i in range(n.value)];self.x.XFree(kids)
   for child in reversed(ids):
    found=self.find(child)
    if found:return found
  return None
 def move(self,w,x,y):
  gx=c.c_int();gy=c.c_int();child=c.c_ulong()
  self.x.XRaiseWindow(self.d,w)
  self.x.XSetInputFocus(self.d,w,1,0)
  self.x.XSync(self.d,0);time.sleep(.1)
  self.x.XTranslateCoordinates(self.d,w,self.root,round(x),round(y),c.byref(gx),c.byref(gy),c.byref(child))
  self.xt.XTestFakeMotionEvent(self.d,-1,gx.value,gy.value,0);self.x.XFlush(self.d)
 def click(self,w,x,y):
  self.move(w,x,y);time.sleep(.04)
  self.xt.XTestFakeButtonEvent(self.d,1,1,0);self.x.XFlush(self.d);time.sleep(.04)
  self.xt.XTestFakeButtonEvent(self.d,1,0,0);self.x.XFlush(self.d);time.sleep(.22)
 def scroll(self,w,x,y,button,count=5):
  self.move(w,x,y)
  for _ in range(count):
   self.xt.XTestFakeButtonEvent(self.d,button,1,0);self.xt.XTestFakeButtonEvent(self.d,button,0,0)
  self.x.XFlush(self.d);time.sleep(.3)
 def key(self,keysym):
  code=self.x.XKeysymToKeycode(self.d,keysym)
  self.xt.XTestFakeKeyEvent(self.d,code,1,0);self.xt.XTestFakeKeyEvent(self.d,code,0,0)
  self.x.XFlush(self.d);time.sleep(.15)
 def type(self,text):
  for char in text:
   code=self.x.XKeysymToKeycode(self.d,ord(char));self.xt.XTestFakeKeyEvent(self.d,code,1,0);self.xt.XTestFakeKeyEvent(self.d,code,0,0)
  self.x.XFlush(self.d);time.sleep(.15)
 def screenshot(self,w,path,width,height):
  try:from PIL import Image
  except ImportError:return
  class XImage(c.Structure):
   _fields_=[('width',c.c_int),('height',c.c_int),('xoffset',c.c_int),('format',c.c_int),('data',c.c_void_p),('byte_order',c.c_int),('bitmap_unit',c.c_int),('bitmap_bit_order',c.c_int),('bitmap_pad',c.c_int),('depth',c.c_int),('bytes_per_line',c.c_int),('bits_per_pixel',c.c_int)]
  self.x.XGetImage.argtypes=[c.c_void_p,c.c_ulong,c.c_int,c.c_int,c.c_uint,c.c_uint,c.c_ulong,c.c_int];self.x.XGetImage.restype=c.POINTER(XImage)
  image=self.x.XGetImage(self.d,w,0,0,width,height,c.c_ulong(-1).value,2)
  if image:
   v=image.contents;data=c.string_at(v.data,v.bytes_per_line*v.height)
   Image.frombytes('RGB',(v.width,v.height),data,'raw','BGRX',v.bytes_per_line).save(path)
   self.x.XDestroyImage.argtypes=[c.POINTER(XImage)];self.x.XDestroyImage(image)

def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--size',default='1200x850');parser.add_argument('--skip-refresh',action='store_true');args=parser.parse_args()
 width,height=map(int,args.size.split('x'));out=ROOT/'build/previews/revision';out.mkdir(parents=True,exist_ok=True)
 with tempfile.TemporaryDirectory(prefix='terramars-pointer-') as temp:
  trace=Path(temp)/'state.json';log=(out/f'pointer-{args.size}.log').open('w');env=os.environ.copy();env['WAYLAND_DISPLAY']=''
  app=subprocess.Popen([str(ROOT/'target/release/terramars'),'--interaction-test',str(trace),'--window-size',args.size],cwd=ROOT,env=env,stdout=log,stderr=log)
  checks=[]
  try:
   d=Desktop(app.pid)
   def state():
    try:return json.loads(trace.read_text())
    except (OSError,json.JSONDecodeError):return {'state':{},'boxes':{}}
   def until(predicate,timeout=8):
    deadline=time.monotonic()+timeout
    while time.monotonic()<deadline:
     if app.poll() is not None:raise AssertionError(f'App exited {app.returncode}: {(out/f"pointer-{args.size}.log").read_text()}')
     value=state()
     if predicate(value):return value
     time.sleep(.05)
    raise AssertionError(f'Timed out; current state: {state()["state"]}')
   until(lambda v:v['state'].get('page')=='home',45);time.sleep(1.5);w=d.find();assert w
   def check(name,condition):
    assert condition,name;checks.append(name);print('PASS',name,flush=True)
   def target(token):
    data=until(lambda v:any(token in k and not k.startswith(('button-label/','map-label/')) for k in v['boxes']))
    key=next(k for k in data['boxes'] if token in k and not k.startswith(('button-label/','map-label/')))
    return data['boxes'][key]
   def visible(b):
    clip=b.get('clip',{'x':0,'y':0,'w':width,'h':height})
    x=max(b['x'],clip['x'],0);y=max(b['y'],clip['y'],82)
    right=min(b['x']+b['w'],clip['x']+clip['w'],width);bottom=min(b['y']+b['h'],clip['y']+clip['h'],height)
    if b['y']<82: y=max(b['y'],clip['y'],0)
    return (x,y,right-x,bottom-y)
   def click(token,condition=None):
    for attempt in range(14):
     b=target(token);x,y,bw,bh=visible(b)
     if bw>=min(12,b['w']) and bh>=min(24,b['h']):break
     st=state()['state'];direction=4 if b['y']<max(b.get('clip',{}).get('y',82),82) else 5
     if st.get('endpoint_menu') is not None:d.scroll(w,26,height-110,direction,3)
     elif st.get('page')=='log':d.scroll(w,width*.5,height*.6,direction,3)
     else:
      title=target('toggle-planner');d.scroll(w,title['x']-70,title['y']+88,direction,3)
    else:raise AssertionError(f'Button remains clipped: {token} {b}')
    d.click(w,x+bw/2,y+bh/2)
    if condition:until(lambda v:condition(v['state']))
   def shot(name):
    time.sleep(.3);d.screenshot(w,out/f'{args.size}-{name}.png',width,height);(out/f'{args.size}-{name}-state.json').write_text(json.dumps(state(),indent=2))
   check('motion and microphone options removed',not any('motion' in k or 'voice' in k for k in state()['boxes']))
   click('units-scientific',lambda s:s['units']=='Scientific');click('units-metric',lambda s:s['units']=='Metric');check('segmented unit choices respond',True);shot('home')
   label_boxes=[(k,b) for k,b in state()['boxes'].items() if k.startswith('button-label/')]
   check('shared button labels are vertically centered',bool(label_boxes) and all(abs((b['y']+b['h']/2)-(state()['boxes'][k.removeprefix('button-label/')]['y']+state()['boxes'][k.removeprefix('button-label/')]['h']/2))<=1.1 for k,b in label_boxes))
   hb=target('home');lb=target('mission-log')
   nav_center=(hb['x']+lb['x']+lb['w'])/2
   check('header navigation is centered or compact-left',abs(nav_center-width/2)<3 if width>=1150 else hb['x']<25)
   before=state()['state']['earth'];click('clock',lambda s:s['earth']!=before);check('clock arrow control switches time',True)
   d.click(w,width*.27,height*.5);until(lambda v:v['state'].get('page')=='explorer')
   check('region opens with panels hidden',not state()['state']['details'] and not state()['state']['planner']);shot('map-before-selection')
   d.click(w,width*.5,240);until(lambda v:v['state'].get('selected') is not None)
   original=state()['state']['selected'];check('selected point autofills B',state()['state']['end']==original and state()['state']['start'] is None)
   def control_box(token):
    return next(b for k,b in state()['boxes'].items() if token in k and not k.startswith(('button-label/','map-label/')))
   check('all map controls remain above the open planner',all(control_box(k)['y']+control_box(k)['h']<=control_box('toggle-planner')['y']-12 for k in ('zoom-in','zoom-out','fit-map')))
   shot('selection')
   try:
    from PIL import Image
    im=Image.open(out/f'{args.size}-selection.png').convert('RGB');b=control_box('toggle-planner');pixels=[]
    for py in range(round(b['y'])+3,round(b['y']+b['h'])-3):
     for px in range(round(b['x'])+3,round(b['x']+b['w'])-3):
      if min(im.getpixel((px,py)))>210:pixels.append((px,py))
    assert pixels,'Chevron must be visible'
    cx=(min(p[0] for p in pixels)+max(p[0] for p in pixels))/2;cy=(min(p[1] for p in pixels)+max(p[1] for p in pixels))/2
    check('planner chevron is visually centered',abs(cx-b['x']-b['w']/2)<2 and abs(cy-b['y']-b['h']/2)<2)
   except ImportError:pass
   click('start-point',lambda s:s['endpoint_menu']=='Start');shot('picker')
   click('landmark-option',lambda s:s['endpoint_menu'] is None and s['start'] is not None)
   check('endpoint menu selects without moving inspected point',state()['state']['selected']==original)
   click('toggle-planner',lambda s:not s['planner']);click('toggle-planner',lambda s:s['planner']);check('planner collapses and reopens with actual clicks',True)
   click('end-point',lambda s:s['endpoint_menu']=='End')
   # Distinct sourced endpoint: tuple debug formatting includes index1.
   data=state();key=next(k for k in data['boxes'] if 'landmark-option' in k and ', 1)' in k);b=data['boxes'][key];d.click(w,b['x']+b['w']/2,b['y']+b['h']/2)
   until(lambda v:v['state']['endpoint_menu'] is None)
   click('calculate-route',lambda s:s['log_count']>=1 and s['route_count']>=1)
   check('route plan is logged through the real button',True)
   # Rover mode follows the published drive, on foot keeps the direct screened line.
   click('travel-mode');until(lambda v:'Recorded rover corridor' in v['state']['projected_time'],6);check('rover mode follows the recorded corridor',True)
   click('travel-mode');until(lambda v:'On foot' in v['state']['projected_time'] and 'direct' in v['state']['projected_time'],6)
   # Repeated mode taps recalculate but never append duplicate mission history.
   log_before=state()['state']['log_count']
   for _ in range(4):click('travel-mode');time.sleep(.05)
   time.sleep(1.0)
   check('mode recalculations do not duplicate mission history',state()['state']['log_count']==log_before)
   if 'Rover' in state()['state']['projected_time']:click('travel-mode')
   until(lambda v:'On foot' in v['state']['projected_time'])
   for _ in range(3):
    count=state()['state']['log_count'];click('calculate-route',lambda s:s['log_count']==count+1)

   # An older notification timer must not dismiss a newer identical message.
   time.sleep(2.7)
   count=state()['state']['log_count'];click('calculate-route',lambda s:s['log_count']==count+1)
   time.sleep(3)
   check('a new notification restarts its five-second lifetime',state()['state']['status'].startswith('Route planned'))
   until(lambda v:v['state']['status']=='',4)
   check('notification disappears without removing mission history',state()['state']['log_count']==count+1)

   if width<900:
    clipped=next((k for k,b in state()['boxes'].items() if k.startswith('button-label/') and b['detail']['overflow']>2),None)
    assert clipped,'Expected a long route endpoint in compact layout'
    offset=state()['boxes'][clipped]['detail']['offset'];until(lambda v:abs(v['boxes'].get(clipped,{}).get('detail',{}).get('offset',offset)-offset)>2)
    check('overflowing labels actually scroll',True)
   check('route summary fills the planner header',bool(state()['state']['projected_time']) and 'plan a route' not in state()['state']['projected_time'])
   click('fit-map');until(lambda v:v['state']['zoom']>1.1);check('fit map frames the planned route',True)
   click('layers-tab',lambda s:s['layers'] and s['details']);data=state();before=data['state']['map_options']['terrain']
   click('map-layer',lambda s:s['map_options']['terrain']!=before);check('right-hand layer switch responds',True);shot('layers')
   click('conditions-tab',lambda s:not s['layers']);zoom=state()['state']['zoom']
   d.scroll(w,width-170,min(height-90,450),5,15);d.scroll(w,width-170,min(height-90,450),4,25)
   check('scrolling location panel never zooms map',abs(state()['state']['zoom']-zoom)<1e-5)
   click('show-details',lambda s:not s['details']);check('Info closes without selecting the map',state()['state']['selected']==original)
   click('show-details',lambda s:s['details']);click('show-details',lambda s:not s['details']);check('top location toggle opens and closes',True)
   click('start-point',lambda s:s['endpoint_menu']=='Start');click('coordinate-input',lambda s:s['coordinate_editing']);d.type('18.44,77.45');until(lambda v:v['state']['coordinate_text']=='18.44,77.45');click('apply-coordinates',lambda s:s['endpoint_menu'] is None and abs(s['start']['latitude']-18.44)<1e-7)
   check('typed coordinates can be chosen',state()['state']['selected']==original)
   # The picker fades out instead of vanishing instantly, so wait until it has
   # left the tree before reopening it, the way a person would.
   until(lambda v:not any(k.startswith('Name("close-endpoint-menu")') for k in v['boxes']))
   click('start-point',lambda s:s['endpoint_menu']=='Start');click('close-endpoint-menu',lambda s:s['endpoint_menu'] is None);check('endpoint picker X closes',True)
   click('start-point',lambda s:s['endpoint_menu']=='Start');click('pick-on-map',lambda s:s['endpoint_menu'] is None);d.click(w,width*.65,240);until(lambda v:abs(v['state']['start']['latitude']-18.44)>1e-7);check('map endpoint picking preserves inspected point',state()['state']['selected']==original)
   check('rover focus and export controls removed',not any('focus-rover' in k or 'export-route' in k for k in state()['boxes']))
   click('fit-map');until(lambda v:abs(v['state']['zoom']-1.)<.01);check('fit map button responds',True)
   d.scroll(w,width*.5,240,4,2);until(lambda v:v['state']['zoom']>1.05);check('scrolling uncovered map still zooms',True)
   if not args.skip_refresh:
    click('refresh');until(lambda v:v['state'].get('refreshing') is False and v['state'].get('status','').startswith('NASA'),45)
    if any('refresh-details' in k for k in state()['boxes']):
     click('refresh-details',lambda s:s['refresh_expanded']);shot('refresh');click('refresh-details',lambda s:not s['refresh_expanded'])
    click('dismiss-status',lambda s:s['status']=='');check('source notification expands and dismisses reliably',True)
   click('start-point',lambda s:s['endpoint_menu']=='Start')
   browsing={k:state()['state'][k] for k in ('selected','start','end','zoom','camera_center','bearing','planner','details','map_options','endpoint_menu','coordinate_text','coordinate_editing')}
   click('mission-log',lambda s:s['page']=='log');shot('log')
   click('open-log-route',lambda s:s['log_preview'] and s['page']=='log');shot('saved-route')
   check('saved route opens within log without mutating explorer',all(state()['state'][k]==v for k,v in browsing.items()))
   click('close-log-preview',lambda s:not s['log_preview'] and s['page']=='log')
   last=next(k for k in state()['boxes'] if 'open-log-route' in k and ', 3)' in k and not k.startswith('button-label/'))
   click(last,lambda s:s['log_preview']);click('close-log-preview',lambda s:not s['log_preview']);check('mission log cards keep their height and scroll',True)
   click('open-log-route',lambda s:s['log_preview']);d.key(0xff1b);until(lambda v:not v['state']['log_preview']);check('Escape closes only the log preview',state()['state']['endpoint_menu']==browsing['endpoint_menu'])
   click('clear-log',lambda s:s['log_count']==0);check('mission log header clears every saved route',True)
   click('home',lambda s:s['page']=='explorer');check('return from log restores complete browsing state',all(state()['state'][k]==v for k,v in browsing.items()))
   click('close-endpoint-menu',lambda s:s['endpoint_menu'] is None)
   click('toggle-planner',lambda s:not s['planner'])
   before_selection=state()['state']['selected']
   for quadrant in (1,2,3,0):
    click('compass');until(lambda v:abs((v['state']['bearing']%(2*3.141592653589793))-quadrant*3.141592653589793/2)<.02 or (quadrant==0 and abs(v['state']['bearing']%(2*3.141592653589793)-2*3.141592653589793)<.02))
    if quadrant==1:shot('rotated-east')
   check('compass cycles all cardinal directions without selecting map',state()['state']['selected']==before_selection)
   click('fit-map');until(lambda v:abs(v['state']['zoom']-1.)<.01)
   data=state();danger=next((k for k,b in data['boxes'].items() if k.startswith('danger/') and 270<b['y']<height-150 and 90<b['x']<width-380),None)
   assert danger,'A real visible MOLA danger zone is required for this regression'
   if danger:
    click(danger,lambda s:s['danger_selected'] and s['details']);shot('danger');check('danger click opens terrain explanation',True)
    click('show-details',lambda s:not s['details'])
    click('toggle-planner',lambda s:not s['planner'])
   click('landmark/Perseverance');until(lambda v:v['boxes'].get('map-label/Perseverance',{}).get('detail',{}).get('color')==0xff945e)
   check('selected landmark uses one orange name without Selected overlap','map-label/Selected' not in state()['boxes']);shot('selected-landmark')
   click('back-to-regions',lambda s:s['page']=='home');check('crater back arrow returns to region chooser',state()['state']['status']=='')
   result={'passed':True,'checks_count':len(checks),'checks':checks,'size':args.size,'event_source':'XTest real pointer and keyboard events'}
   (out/f'pointer-{args.size}.json').write_text(json.dumps(result,indent=2)+'\n')
  except Exception:
   try:
    shot('failure');(out/f'failure-{args.size}.json').write_text(json.dumps(state(),indent=2))
   except Exception:pass
   raise
  finally:
   app.terminate()
   try:app.wait(timeout=5)
   except subprocess.TimeoutExpired:app.kill()
if __name__=='__main__':main()
