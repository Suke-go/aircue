"""Convert MIT KEMAR diffuse-field-equalized WAV HRIRs into the embedded 48 kHz bank.
Usage: python tools/build_hrtf.py path/to/extracted/diffuse assets/kemar.bin
Source: https://sound.media.mit.edu/resources/KEMAR/diffuse.zip
"""
import sys,re,wave,struct,pathlib
import numpy as np
from scipy.signal import resample_poly
root=pathlib.Path(sys.argv[1]); records=[]
for p in root.rglob('*.wav'):
 m=re.fullmatch(r'H(-?\d+)e(\d+)a.wav',p.name)
 if not m:continue
 el,az=map(int,m.groups())
 with wave.open(str(p)) as w:
  assert w.getframerate()==44100 and w.getnchannels()==2 and w.getsampwidth()==2
  data=np.frombuffer(w.readframes(w.getnframes()),dtype='<i2').reshape(-1,2)/32768.
 # FIR resampling also needs the rate-ratio factor to preserve its transfer gain.
 data=resample_poly(data,160,147,axis=0)*147/160
 records.append((az,el,data))
records.sort(key=lambda x:(x[1],x[0])); n=len(records[0][2]);assert all(len(x[2])==n for x in records)
with open(sys.argv[2],'wb') as f:
 f.write(struct.pack('<4sIII',b'AKHR',48000,len(records),n))
 for az,el,data in records:f.write(struct.pack('<ff',az,el));f.write(data.astype('<f4').tobytes())
print(len(records),'directions,',n,'taps per ear, 48 kHz')
