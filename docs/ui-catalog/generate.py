#!/usr/bin/env python3
"""Generate bilingual editor documentation and static SVG/HTML design artefacts.

Python 3.9+ standard library only. Run from any directory. No editor, compiler,
GPU, authored project, user layout or external network operation is performed.
The maintained inputs are catalog.json and ../editor-ui-tokens.json.
"""
import argparse
import hashlib
import html
import json
import math
import os
import re
from pathlib import Path
from hierarchy import Hierarchy, ICONS as HIERARCHY_ICONS

ROOT = Path(__file__).resolve().parents[2]
DOCS = ROOT / 'docs'
DATA = json.loads((DOCS / 'ui-catalog/catalog.json').read_text(encoding='utf-8'))
TOKENS = json.loads((DOCS / 'editor-ui-tokens.json').read_text(encoding='utf-8'))
C = TOKENS['color']
MEDIA = DOCS / 'media/editor-ui-catalog'
GROUPS = {
 'windows': ('Editor windows and workspaces', 'Окна и раскладки редактора', ('shell','panes','modules')),
 'views': ('Embedded views and shared controls', 'Вложенные виды и общие элементы', ('views','components')),
 'dialogs': ('Dialogs and native picker purposes', 'Диалоги и цели системных picker', ('dialogs','native')),
 'menus': ('Menus and contextual commands', 'Меню и контекстные команды', ('menus',))}
STATUS = {
 'current_surface': ('Current surface; proposed layout', 'Существующий интерфейс; предлагаемая вёрстка'),
 'proposed_surface': ('Proposed surface; not implemented', 'Предложенный интерфейс; не реализован'),
 'extension_contract': ('Future extension contract, not a real registration', 'Контракт будущего расширения, не регистрация'),
 'shared_component': ('Shared control family; proposed standard', 'Общее семейство элементов; предлагаемый стандарт')}

def language(value, lang):
    return value[lang]

def esc(value):
    return html.escape(str(value), quote=True)

def group_for(entry):
    return next(k for k,v in GROUPS.items() if entry['group'] in v[2])

def md(value):
    return value.replace('|', '\\|').replace('\n', '<br>')

def markdown_blocks(parts):
    # GFM tables require adjacent rows; prose/list blocks keep blank separators.
    return re.sub(r'(?m)(^\|.*\|)\n\n(?=\|)',r'\1\n','\n\n'.join(parts))+'\n'

def estimate(text, size):
    # Conservative layout estimate, independently checked with rendered fonts.
    weights = []
    for ch in text:
        if ch.isspace(): weights.append(.34)
        elif ch in 'MWmw@%&': weights.append(1.0)
        elif ch in 'ilI.,:;!|\'`': weights.append(.35)
        elif ch.isupper(): weights.append(.75)
        else: weights.append(.62)
    return sum(weights) * size

def wrap(text, width, size=14):
    result = []
    for paragraph in str(text).split('\n'):
        words = paragraph.split(' ')
        line = ''
        for word in words:
            if estimate(word, size) > width:
                if line: result.append(line); line = ''
                chunk = ''
                for ch in word:
                    if chunk and estimate(chunk+ch, size) > width:
                        result.append(chunk); chunk = ''
                    chunk += ch
                line = chunk
            elif line and estimate(line+' '+word, size) > width:
                result.append(line); line = word
            else:
                line += (' ' if line else '') + word
        result.append(line)
    return result or ['']

ICON = {
 'play':'<path d="M8 4 L20 12 L8 20 Z"/>',
 'pause':'<path d="M7 4 V20 M17 4 V20"/>',
 'stop':'<rect x="5" y="5" width="14" height="14" rx="1"/>',
 'save':'<path d="M3 3 H17 L21 7 V21 H3 Z M7 3 V9 H16 V3 M7 21 V14 H17 V21"/>',
 'move':'<path d="M12 2 V22 M2 12 H22 M8 6 L12 2 L16 6 M8 18 L12 22 L16 18 M6 8 L2 12 L6 16 M18 8 L22 12 L18 16"/>',
 'rotate':'<path d="M20 11 A8 8 0 1 1 17 6 M20 3 V9 H14"/>',
 'scale':'<path d="M3 21 L20 4 M12 4 H20 V12 M3 13 V21 H11"/>',
 'frame':'<path d="M3 9 V3 H9 M15 3 H21 V9 M21 15 V21 H15 M9 21 H3 V15"/>',
 'snap':'<path d="M4 4 V13 A8 8 0 0 0 20 13 V4 H15 V13 A3 3 0 0 1 9 13 V4 Z M4 9 H9 M15 9 H20"/>',
 'folder':'<path d="M2 5 H10 L12 8 H22 V20 H2 Z"/>',
 'mesh':'<path d="M12 2 L22 7 V17 L12 22 L2 17 V7 Z M2 7 L12 12 L22 7 M12 12 V22"/>',
 'chevron':'<path d="M6 9 L12 15 L18 9"/>',
 'chevron_right':'<path d="M9 6 L15 12 L9 18"/>',
 'restart':'<path d="M20 11 A8 8 0 1 1 17 6 M20 3 V9 H14 M10 9 L15 12 L10 15 Z"/>',
 'more':'<circle cx="5" cy="12" r="1"/><circle cx="12" cy="12" r="1"/><circle cx="19" cy="12" r="1"/>',
}
ICON.update(HIERARCHY_ICONS)

class SVG:
    def __init__(self, entry, state_sheet=False, compact=False):
        self.entry = entry
        self.compact = compact
        self.w, self.h = (480,900) if compact else (1280, 1024) if state_sheet else (1280, 900)
        self.parts = []
        self.boxes = []
        self.rect(0,0,self.w,self.h,C['canvas'])
        self.text(16,30,entry['id']+' / '+entry['label']['en'],20,w=self.w-32,weight=600)
        caption='DESIGN PROPOSAL · 480-point dock area' if compact else 'DESIGN PROPOSAL · Illustrative data · '+STATUS[entry['status']][0]
        self.text(16,54,caption,12,C['secondary'],w=self.w-32)

    def rect(self,x,y,w,h,fill=None,stroke=None,r=0,sw=1):
        self.parts.append(f'<rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="{fill or C["panel"]}" stroke="{stroke or "none"}" stroke-width="{sw}"/>')

    def line(self,x1,y1,x2,y2,color=None,width=1):
        self.parts.append(f'<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke="{color or C["separator"]}" stroke-width="{width}"/>')

    def text(self,x,y,value,size=14,color=None,w=1200,weight=400,code=False):
        lines = wrap(value,w,size)
        family = 'Consolas, monospace' if code else 'Segoe UI, Arial, sans-serif'
        for i,line in enumerate(lines):
            baseline = y+i*20
            self.parts.append(f'<text x="{x}" y="{baseline}" font-family="{family}" font-size="{size}" font-weight="{weight}" fill="{color or C["text"]}" data-max-width="{w}">{esc(line)}</text>')
            self.boxes.append(dict(x=x,baseline=baseline,width=w,size=size,text=line,code=code))
        return y+len(lines)*20

    def icon(self,name,x,y,size=20,color=None):
        self.parts.append(f'<g transform="translate({x} {y}) scale({size/24})" fill="none" stroke="{color or C["icon"]}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{ICON[name]}</g>')

    def button(self,x,y,label,w=None,primary=False,icon=None,selected=False,disabled=False,h=None):
        if icon:
            w = w or 32
        else:
            w = w or min(300,max(88,math.ceil(estimate(label,14))+24))
        h=h or (TOKENS['control']['icon_button'] if icon else TOKENS['control']['height'])
        self.rect(x,y,w,h,C['panel'] if disabled else C['selected'] if selected else C['control'],C['separator'] if disabled else C['accent'] if primary or selected else C['boundary'],4)
        if icon:
            self.icon(icon,x+(w-20)/2,y+(h-20)/2,color=C['disabled'] if disabled else None)
        else:
            self.text(x+12,y+(h+14)/2-2,label,14,C['disabled'] if disabled else None,w=w-24)
        if selected:self.rect(x+2,y+4,3,h-8,C['accent'])
        return w

    def toolbar(self,x,y,w,actions,use_icons=True):
        cursor=x
        icon_map={'Play':'play','Pause':'pause','Stop':'stop','Save Project':'save','Restart':'restart','Start preview':'play','Pause preview':'pause','Reset preview':'restart',
                  'Move':'move','Rotate':'rotate','Scale':'scale','Frame Selection':'frame','Snap':'snap'}
        for label in actions:
            icon=icon_map.get(label) if use_icons else None
            needed=32 if icon else min(300,max(88,math.ceil(estimate(label,14))+24))
            if cursor+needed > x+w-48:
                self.button(x+w-36,y,'',32,icon='more'); break
            cursor += self.button(cursor,y if icon else y+2,label,needed,icon=icon)+8

    def panel(self,x,y,w,h,title):
        self.rect(x,y,w,h,C['panel'],C['separator'],4)
        self.rect(x,y,w,32,C['raised'])
        self.text(x+12,y+21,title,14,w=w-48,weight=600)
        self.icon('chevron',x+w-28,y+6)

    def rows(self,x,y,w,rows,row_height=64):
        for row in rows:
            cells=row.split('|')
            self.line(x,y+row_height-4,x+w,y+row_height-4)
            if len(cells)==1:
                self.text(x+8,y+23,cells[0],14,w=w-16)
            else:
                label_w=min(280,w*.32)
                self.text(x+8,y+22,cells[0],12,C['secondary'],w=label_w-16)
                self.text(x+label_w,y+23,cells[1],14,w=w-label_w-16,weight=600)
                if len(cells)>2:
                    self.text(x+label_w,y+44,' · '.join(cells[2:]),12,C['secondary'],w=w-label_w-16)
            y+=row_height
        return y

    def form_rows(self,x,y,w,rows,row_height=80):
        for row in rows:
            cells=row.split('|')
            label_w=min(280,w*.30)
            self.text(x+8,y+22,cells[0],12,C['secondary'],w=label_w-16)
            value=cells[1] if len(cells)>1 else 'Source-defined value'
            field_w=min(600,w-label_w-16)
            fh=max(28,8+20*len(wrap(value,field_w-40)))
            self.rect(x+label_w,y+4,field_w,fh,C['control'],C['boundary'],4)
            self.text(x+label_w+8,y+23,value,14,w=field_w-40)
            if cells[0] in ('Boolean','Enum','ObjectReference','AssetReference','Prefab','Variants') or 'choices' in cells[0].lower():
                self.icon('chevron',x+label_w+field_w-28,y+8)
            bottom=y+fh+12
            if len(cells)>2:bottom=self.text(x+label_w,y+fh+24,' · '.join(cells[2:]),12,C['secondary'],w=w-label_w-16)
            actual=max(row_height,bottom-y+8)
            self.line(x,y+actual-4,x+w,y+actual-4)
            y+=actual
        return y

    def widgets(self,x,y,w,items,compact=False):
        for item in items:
            label=item['label'];value=item['value'];kind=item['kind'];meta=item.get('meta','')
            if compact:
                self.text(x+8,y+18,label,12,C['secondary'],w=w-16)
                fx=x+8;fy=y+28;fw=w-16
            else:
                lw=min(240,w*.27)
                self.text(x+8,y+19,label,12,C['secondary'],w=lw-16)
                fx=x+lw;fy=y;fw=min(720,w-lw-16)
            if kind=='readonly':
                after=self.text(fx+8,fy+19,str(value),14,w=fw-16)
            elif kind=='boolean':
                self.rect(fx+8,fy+5,18,18,C['selected'] if value else C['control'],C['boundary'],2)
                if value:self.parts.append(f'<path d="M{fx+11} {fy+14} l4 4 l8 -9" fill="none" stroke="{C["accent"]}" stroke-width="2"/>')
                self.text(fx+36,fy+19,'Enabled' if value else 'Disabled',14,w=fw-44);after=fy+39
            elif kind=='vector3':
                axis=['Default','Min','Max'] if label.startswith('Default /') else ['X','Y','Z']
                cell=(fw-16)/3
                for i,v in enumerate(value):
                    cx=fx+i*(cell+8)
                    self.text(cx+8,fy+19,axis[i],12,C['secondary'],w=64 if axis[0]=='Default' else 20)
                    ax=72 if axis[0]=='Default' else 28
                    self.rect(cx+ax,fy,cell-ax,28,C['control'],C['boundary'],4)
                    self.text(cx+ax+8,fy+19,str(v),14,w=cell-ax-16)
                after=fy+40
            else:
                reserve=40 if kind=='enum' else 16
                size=13 if kind=='code' else 14
                height=max(28,8+20*len(wrap(str(value),fw-reserve,size)))
                self.rect(fx,fy,fw,height,C['control'],C['boundary'],4)
                after=self.text(fx+8,fy+19,str(value),size,w=fw-reserve,code=kind=='code')
                if kind=='enum':self.icon('chevron',fx+fw-28,fy+4)
            if meta:after=self.text(fx+8,max(after,fy+40),meta,12,C['secondary'],w=fw-16)
            actual=4*math.ceil(max(56,after-y+12)/4)
            self.line(x,y+actual-4,x+w,y+actual-4)
            y+=actual
        return y

    def editor_frame(self):
        self.rect(16,72,1248,792,C['panel'],C['boundary'],4)
        self.rect(16,72,1248,32,C['raised'])
        for x,label in [(28,'File'),(68,'Edit'),(108,'Tools'),(160,'Windows')]: self.text(x,93,label,14,w=88)
        project_view=self.entry['id'] in ('S01','P06','P07','P08','P09','P10','P21','V03','D12')
        for tx,label in [(336,'Engine'),(432,'Project'),(528,'My layout')]:
            active=(label=='Project' if project_view else label=='Engine')
            self.rect(tx,74,88 if label!='My layout' else 112,28,C['selected'] if active else C['raised'],C['accent'] if active else 'none',4)
            if active:self.rect(tx+2,100,84,2,C['accent'])
            self.text(tx+12,93,label,14,w=88 if label=='My layout' else 64)
        self.text(656,94,'+',20,w=28)
        self.rect(16,104,1248,36,C['panel'])
        self.text(28,128,'Example project · Full · matching sources',12,C['secondary'],w=400)
        if self.entry['group']=='components':
            self.text(500,128,'Read-only component samples',12,C['secondary'],w=390)
        elif self.entry['id']=='P10':
            self.parts.append('<g data-command-owner="shell" data-command-target="game-session">')
            self.toolbar(500,106,172,['Pause','Stop','Restart'])
            self.parts.append('</g>')
            self.line(628,112,628,132)
            self.button(644,106,'Save Project',32,icon='save')
        else:
            self.toolbar(500,106,390,['Play','Stop','Save Project'])
        self.text(936,128,'Playing · game session' if self.entry['id']=='P10' else 'Authored / applied status',12,C['secondary'],w=300)
        self.panel(16,140,1248,700,self.entry['label']['en'])
        self.rect(16,840,1248,24,C['raised'])
        self.text(28,857,'Ready area · Proposed interface, not a running editor capture',12,C['secondary'],w=1220)
        return 28,184,1224,640

    def graph(self,x,y,w,h,rows,kind):
        self.rect(x,y,w,h,C['canvas'],C['separator'],4)
        for gx in range(int(x)+12,int(x+w),24):
            for gy in range(int(y)+12,int(y+h),24):
                self.parts.append(f'<circle cx="{gx}" cy="{gy}" r=".7" fill="{C["separator"]}"/>')
        if kind=='rust_graph' and self.entry['id']=='V12':
            self.scalar_nodes(x,y,w,h,rows)
            return
        ncols=4 if len(rows)>4 else max(1,len(rows))
        nw=min(220,(w-40-(ncols-1)*28)/ncols)
        positions=[]
        for i,row in enumerate(rows):
            gx=x+20+(i%ncols)*(nw+28); gy=y+40+(i//ncols)*160
            cells=row.split('|')
            self.rect(gx,gy,nw,104,C['panel'],C['boundary'],4)
            self.rect(gx,gy,nw,28,C['raised'])
            self.text(gx+8,gy+19,cells[0],14,w=nw-16,weight=600)
            self.text(gx+12,gy+58,' / '.join(cells[1:]) or 'Supported node',12,C['secondary'],w=nw-24)
            for px,side in [(gx,'in'),(gx+nw,'out')]:
                if kind=='gpu_graph' and ((i==0 and side=='in') or (i==len(rows)-1 and side=='out')):continue
                color=C['pin_scene']
                if kind=='gpu_graph':
                    color=C['pin_scene'] if (i==0 or (i==1 and side=='in')) else C['pin_color']
                elif kind=='rust_graph' and self.entry['id']=='V12':
                    color=C['pin_scalar']
                self.parts.append(f'<circle cx="{px}" cy="{gy+80}" r="5" fill="{color}"/>')
            positions.append((gx,gy,nw))
        for i in range(len(positions)-1):
            a,b=positions[i],positions[i+1]
            color=C['pin_scene']
            if kind=='gpu_graph' and i>0:color=C['pin_color']
            elif kind=='rust_graph' and self.entry['id']=='V12':color=C['pin_scalar']
            sx,sy=a[0]+a[2],a[1]+80
            tx,ty=b[0],b[1]+80
            if i%ncols==ncols-1:
                self.parts.append(f'<path d="M{sx} {sy} H{sx+12} V{a[1]+128} H{tx-12} V{ty} H{tx}" fill="none" stroke="{color}" stroke-width="2"/>')
            else:
                self.line(sx,sy,tx,ty,color,2)
        if kind=='gpu_graph':
            ao=next((i for i,row in enumerate(rows) if 'SSAO' in row or 'Ambient occlusion' in row),None)
            targets=([2,ao] if self.entry['id']=='V11' else [ao]) if ao is not None else []
            a=positions[1]
            for target in targets:
                b=positions[target]
                for offset,key in [(64,'pin_scene'),(96,'pin_depth')]:
                    color=C[key];sx=a[0]+a[2];sy=a[1]+offset;tx=b[0];ty=b[1]+offset
                    if target==2:
                        self.line(sx,sy,tx,ty,color,2)
                    else:
                        route_y=min(a[1],b[1])-(16 if offset==64 else 30)
                        self.parts.append(f'<path d="M{sx} {sy} H{sx+12} V{route_y} H{tx-12} V{ty} H{tx}" fill="none" stroke="{color}" stroke-width="2"/>')
                    for xx,yy in [(sx,sy),(tx,ty)]:self.parts.append(f'<circle cx="{xx}" cy="{yy}" r="5" fill="{color}"/>')
            self.text(x+16,y+h-48,'SSAO uses color + normals + depth. Deferred lighting also consumes G-buffer resources.',12,C['secondary'],w=w-32)
        self.text(x+16,y+h-24,'Illustrative topology · Actual ports and validation stay authoritative',12,C['secondary'],w=w-32)

    def scalar_nodes(self,x,y,w,h,rows):
        color=C['pin_scalar'];nw=220
        positions=[(x+20,y+56),(x+20,y+248),(x+400,y+152),(x+780,y+152)]
        for i,(gx,gy) in enumerate(positions):
            cells=rows[i].split('|')
            self.rect(gx,gy,nw,104,C['panel'],C['boundary'],4)
            self.rect(gx,gy,nw,28,C['raised'])
            self.text(gx+8,gy+19,cells[0],14,w=nw-16,weight=600)
            self.text(gx+12,gy+50,cells[1],12,C['secondary'],w=nw-24)
            inputs=[64,88] if i==2 else [80] if i==3 else []
            for j,offset in enumerate(inputs):
                self.parts.append(f'<circle cx="{gx}" cy="{gy+offset}" r="5" fill="{color}"/>')
                if i==2:self.text(gx+12,gy+offset+4,'a' if j==0 else 'b',12,C['secondary'],w=20)
            if i!=3:self.parts.append(f'<circle cx="{gx+nw}" cy="{gy+80}" r="5" fill="{color}"/>')
        target=positions[2]
        for i,offset in [(0,64),(1,88)]:
            sx,sy=positions[i][0]+nw,positions[i][1]+80
            tx,ty=target[0],target[1]+offset;via=x+300+i*32
            self.parts.append(f'<path d="M{sx} {sy} H{via} V{ty} H{tx}" fill="none" stroke="{color}" stroke-width="2"/>')
        self.line(target[0]+nw,target[1]+80,positions[3][0],positions[3][1]+80,color,2)
        self.text(x+16,y+h-48,'Time and Constant feed separate Multiply inputs. Return has no output.',12,C['secondary'],w=w-32)
        self.text(x+16,y+h-24,'Illustrative supported scalar expression · Actual ports and validation stay authoritative',12,C['secondary'],w=w-32)

    def scene(self,x,y,w,h,game=False,legacy=False):
        self.rect(x,y,w,h,C['canvas'],C['separator'])
        cy=y+h*.60
        self.parts.append(f'<polygon points="{x+w*.20},{cy} {x+w*.46},{y+h*.35} {x+w*.83},{cy} {x+w*.55},{y+h*.88}" fill="{C["raised"]}" stroke="{C["boundary"]}"/>')
        for i in range(3 if legacy else 1):
            cx=x+w*.46+i*82; yy=cy-42-i*10
            self.rect(cx,yy,52,64,C['control'],C['accent'] if not game else C['boundary'])
        if game:
            self.text(x+16,y+28,'GAME CONTENT / HUD IS OWNED BY THE SELECTED PROJECT',12,C['secondary'],w=w-32)
        elif legacy:
            self.text(x+16,y+28,'LEGACY ROOT PREVIEW · camera only; authored Scene tools are elsewhere',12,C['secondary'],w=w-32)
            self.text(x+16,y+h-20,'Orbit camera · wheel distance · Use shared Play/Pause; Reset clears this legacy preview.',12,C['secondary'],w=w-32)
        else:
            self.rect(x+8,y+8,44,124,C['raised'],C['boundary'],4)
            for i,name in enumerate(['move','rotate','scale']): self.button(x+12,y+12+i*40,'',36,icon=name,selected=i==0,h=36)
            self.button(x+60,y+8,'World',112,h=36)
            self.button(x+180,y+8,'Snap 0.5',132,h=36)
            self.button(x+w-48,y+8,'',36,icon='frame',h=36)
            self.text(x+16,y+h-20,'G / R / S tools · F frame · Escape cancels a gesture',12,C['secondary'],w=w-32)

    def body(self,x,y,w,h):
        e=self.entry; rows=e['layout']['rows']; template=e['layout']['template']
        if template in ('menu','file_menu','border_menu'):
            self.menu_body(x,y,w,h); return
        if template in ('dialog','asset_delete','report','preset_review','native_picker','native_overwrite'):
            self.dialog_body(x,y,w,h); return
        if template=='architecture':
            self.toolbar(x,y,w,e['actions'])
            cw=(w-24)/3
            for i,(title,items) in enumerate([
                ('Technical Engine',['World → extraction → rendering','Technical dependencies only']),
                ('Reusable Gameplay',['Selected gameplay packages','Ownership is not execution order']),
                ('Game Project',['Concrete rules / content','Depends on declared contracts'])]):
                self.panel(x+i*(cw+12),y+56,cw,h-112,title)
                self.rows(x+i*(cw+12)+8,y+100,cw-16,items,88)
            self.text(x,y+h-28,'Show only source-backed dependency edges. Unknown evidence is explicit; no invented cross-group order.',12,C['secondary'],w=w)
            return
        if template=='root_graph':
            self.graph(x,y,w,h,rows,template); return
        if template in ('gpu_graph','rust_graph'):
            self.toolbar(x,y,w,e['actions'])
            self.graph(x,y+48,w,h-60,rows,template); return
        if template in ('scene','game','legacy_scene'):
            if template=='legacy_scene':
                self.text(x,y+23,'Preview',14,w=88,weight=600)
                self.parts.append('<g data-command-owner="preview" data-command-target="legacy-root-preview">')
                self.toolbar(x+96,y,w-96,e['actions'])
                self.parts.append('</g>')
            else:self.toolbar(x,y,w,e['actions'] if template!='scene' else [])
            self.scene(x,y+44,w,h-56,template=='game',template=='legacy_scene'); return
        if template=='shell':
            self.panel(x,y,200,h,'Hierarchy'); self.rows(x+4,y+40,192,['Arena|Entry','Hero|Selected','Monsters|Collapsed'],56)
            self.panel(x+208,y,w-512,h-184,'Scene'); self.scene(x+212,y+36,w-520,h-224)
            self.panel(x+208,y+h-176,w-512,176,'Assets');
            for i,label in enumerate(['Characters','Environment','hero.glb']):
                self.icon('folder' if i<2 else 'mesh',x+232+i*144,y+h-126,48)
                self.text(x+224+i*144,y+h-58,label,12,w=132)
            self.panel(x+w-296,y,296,h,'Inspector'); self.rows(x+w-288,y+40,280,['Object|Hero','Position|0.00 0.00 5.00','Prefab|Inherited'],64)
            return
        if template=='external':
            self.rect(x,y,w,h,C['raised'],C['boundary'],4)
            self.text(x+12,y+24,'Separate OS window (proposed) · same document/history',14,w=w-24)
            self.toolbar(x+12,y+40,w-24,['Return to Main Window'])
            self.graph(x+12,y+88,w-24,h-104,['World|Root context','Extract scene|Scene3D','Renderer|Selected module','Viewport|Frame3D'],'root_graph')
            return
        if template in ('assets','trash'):
            self.toolbar(x,y,w,e['actions'])
            self.text(x,y+68,'Project / assets / models     Show address     Copy path',14,w=w)
            self.panel(x,y+88,200,h-100,'Folders')
            for i,label in enumerate(['assets','models','audio']):
                self.icon('folder',x+12+(16 if i else 0),y+132+i*40)
                self.text(x+44+(16 if i else 0),y+148+i*40,label,14,w=132)
            if template=='trash':
                self.rows(x+216,y+100,w-232,rows,72); return
            self.rect(x+216,y+88,480,28,C['control'],C['boundary'],4)
            self.text(x+228,y+107,'Search this folder and descendants',14,C['secondary'],w=456)
            self.button(x+712,y+88,'All types',176)
            for i,label in enumerate(['Characters','Environment','hero.glb']):
                tx=x+228+i*168
                self.rect(tx,y+132,144,128,C['raised'],C['separator'],4)
                self.icon('folder' if i<2 else 'mesh',tx+48,y+148,48)
                self.text(tx+8,y+238,label,14,w=128)
            self.panel(x+920,y+88,w-920,h-100,'Selected asset')
            self.rows(x+928,y+128,w-936,['Name|hero.glb','Identity|asset://stable-id','Preview|Model / texture only'],88)
            self.panel(x+216,y+320,688,160,'Alternate address mode / same folder')
            self.rect(x+228,y+368,664,28,C['control'],C['boundary'],4)
            self.rect(x+236,y+372,112,20,C['selected'])
            self.text(x+236,y+387,'assets/models',14,w=640)
            self.text(x+228,y+428,'Read-only selectable text · Escape/blur returns breadcrumbs.',12,C['secondary'],w=664)
            return
        if template in ('rust_both','rust_text','rust_graph','hlsl','json','state_recovery'):
            self.toolbar(x,y,w,e['actions'])
            if template=='rust_both':
                half=(w-16)/2
                self.code_box(x,y+52,half,h-156,'Rust source / ScriptContext',e['layout'].get('code',''))
                self.graph(x+half+16,y+52,half,h-156,['Text fragment|ctx.time.sin()','Set position|object 1','Set render parameter|tonemap.exposure'],'rust_graph')
            elif template=='hlsl':
                self.panel(x,y+52,320,h-156,'Pass manifest / resources')
                self.rows(x+8,y+96,304,['Pass|customAO','Stage|compute','Entry|main','Input|Declared color resource','Output|Declared storage resource'],64)
                self.code_box(x+336,y+52,w-336,h-156,'HLSL excerpt / match the actual manifest',
                    '[numthreads(8,8,1)]\nvoid main(uint3 id : SV_DispatchThreadID) {\n    // Use the pass-declared bindings.\n    // This is an illustration, not a full shader.\n}')
            elif template=='state_recovery':
                self.widgets(x,y+52,w,e['layout']['widgets'])
                self.text(x+12,y+480,'Keep the working instance until an explicit valid reset/migration succeeds.',14,C['warning'],w=w-24)
            else:
                code=e['layout'].get('code') or next((row.split('|')[1] for row in rows if row.startswith('JSON|') or row.startswith('Rust source|')), '')
                self.code_box(x,y+52,w,h-156,'Selected source excerpt / full data lives in the document',code)
            if template!='state_recovery':
                self.rect(x,y+h-88,w,76,C['raised'],C['boundary'],4)
                self.text(x+12,y+h-62,'Candidate / working revision and line diagnostics',14,w=w-24,weight=600)
                self.text(x+12,y+h-36,'Previous working code / renderer is retained if the candidate is rejected.',12,C['secondary'],w=w-24)
            return
        if template=='overview':
            self.toolbar(x,y,w,e['actions'])
            self.rect(x,y+48,w,32,C['control'],C['boundary'],4)
            self.text(x+12,y+70,'Search capabilities in this project configuration',14,C['secondary'],w=w-24)
            self.panel(x,y+96,200,h-108,'Owner groups')
            self.rows(x+4,y+136,192,['Technical Engine','Gameplay','Project','Unknown'],72)
            self.panel(x+212,y+96,316,h-108,'Capabilities')
            self.rows(x+220,y+136,300,['Rendering|Selected','World storage|Source evidence','Input|Optional capability','Audio|Optional capability'],88)
            self.panel(x+540,y+96,w-540,h-108,'Rendering / evidence and purpose')
            self.rows(x+548,y+136,w-556,['Purpose|Render extracted scene','Installed|Yes · source registry','Offered|Yes · selected profile','Participating|Yes · authored route','Runtime readiness|Not implied by these facts'],76)
            return
        if template=='catalog':
            self.toolbar(x,y,w,e['actions'][1:])
            self.text(x,y+70,'Target: Engine / Rendering / Main',14,C['accent'],w=w)
            self.rect(x,y+88,420,32,C['control'],C['boundary'],4)
            self.text(x+12,y+110,'Search compatible GPU nodes',14,C['secondary'],w=396)
            self.panel(x,y+136,420,h-148,'Available nodes / current scope')
            self.rows(x+8,y+176,404,['Geometry|Forward / G-buffer','Post process|SSAO','Post process|Tone map','Post process|FXAA','Custom pass|HLSL'],76)
            self.panel(x+436,y+88,w-436,h-100,'Selected node / compatibility')
            self.rows(x+444,y+128,w-452,['Selected|FXAA','Purpose|Current-frame anti-aliasing','Inputs / outputs|Actual PassManifest','Add|This graph only; wire explicitly'],80)
            self.text(x+452,y+488,'Pin help is contextual. Connection types are optional reference, not a permanent jargon footer.',14,C['secondary'],w=w-468)
            return
        if template=='systems':
            self.toolbar(x,y,w,e['actions'])
            self.panel(x,y+48,200,h-60,'Stages / resources')
            self.rows(x+4,y+88,192,['Input|First stage','Update|Ordered systems','Render|Extraction','world|GameWorld'],84)
            self.graph(x+212,y+48,w-516,h-60,['Movement|Selected package','Collision|after Movement','Game rules|Project-owned'],'systems')
            self.panel(x+w-292,y+48,292,h-60,'Selected: Collision (example)')
            self.widgets(x+w-284,y+88,276,e['layout'].get('widgets',[]),compact=True)
            return
        if template=='history':
            self.toolbar(x,y,w,e['actions'])
            self.panel(x,y+48,256,h-60,'Saved versions / stable identities')
            self.rows(x+8,y+92,240,['Before render edit','Latest checkpoint','Session Undo / Redo'],64)
            self.widgets(x+272,y+56,w-272,e['layout'].get('widgets',[]))
            return
        if template=='diagnostics':
            self.toolbar(x,y,w,e['actions'])
            self.rect(x,y+48,w,32,C['control'],C['boundary'],4)
            self.text(x+12,y+70,'Search / filter within the current context',14,C['secondary'],w=w-24)
            left=256 if template in ('overview','history') else 0
            if left:
                self.panel(x,y+96,left,h-108,'Groups' if template=='overview' else 'Saved versions')
                self.rows(x+4,y+136,left-8,['Technical Engine' if template=='overview' else 'Before render edit','Gameplay' if template=='overview' else 'Latest checkpoint','Project / Unknown' if template=='overview' else 'Session Undo / Redo'],64)
            self.rows(x+left+(16 if left else 0),y+100,w-left-(16 if left else 0),rows,88)
            return
        if template=='participation':
            self.rect(x,y,w,28,C['control'],C['boundary'],4)
            self.text(x+12,y+19,'Search participating modules',14,C['secondary'],w=w-24)
            self.text(x,y+68,'Panel 100 · owner scene.extract · Engine-owned read-only tool',12,C['secondary'],w=w)
            self.panel(x,y+96,360,h-108,'Effective participating modules')
            self.rect(x+4,y+316,352,84,C['selected'])
            self.rows(x+8,y+136,344,rows[1:4],88)
            self.panel(x+376,y+96,w-376,h-108,'Expanded Gameplay motion / property contract')
            self.rows(x+384,y+136,w-392,[
                'Module ID|game.motion|Selected effective module',
                'Property|Motion speed|game.motion.speed',
                'Scope|Component|game.motion',
                'Type|Number 0–8|Default 1',
                'Evidence|effective_modules(document)|Not a registry-only inventory'],76)
            return
        if template=='grid':
            self.toolbar(x,y,w,['Apply level','Discard draft'])
            fields=e['layout']['widgets']
            self.text(x,y+70,'Garden',12,C['secondary'],w=68)
            self.rect(x+76,y+48,360,28,C['panel'],C['separator'],4)
            self.text(x+84,y+67,fields[0]['value'],14,C['disabled'],w=316)
            self.icon('chevron',x+408,y+52,color=C['disabled'])
            for i,item in enumerate(fields[1:]):
                fx=x+468+i*236
                self.text(fx,y+70,item['label'],12,C['secondary'],w=84)
                self.rect(fx+92,y+48,112,28,C['control'],C['boundary'],4)
                self.text(fx+100,y+67,item['value'],14,w=96)
            self.text(x,y+100,'Garden selection is locked while this draft is modified.',12,C['secondary'],w=w)
            cursor=x
            for brush in ['Floor','Wall','Flower / goal','Crate','Gardener','Erase']:
                cursor+=self.button(cursor,y+116,brush,selected=brush=='Wall')+8
            self.text(x,y+168,'Drag to paint · A goal can sit under a crate · One Apply creates one Undo',12,C['secondary'],w=w)
            cell=60; gx=x+20; gy=y+184
            for yy in range(6):
                for xx in range(8):
                    wall=yy in (0,5) or xx in (0,7)
                    self.rect(gx+xx*cell,gy+yy*cell,cell-2,cell-2,C['control'] if wall else C['raised'],C['boundary'])
                    if (xx,yy)==(3,3): self.icon('mesh',gx+xx*cell+18,gy+yy*cell+18,24)
                    if (xx,yy)==(5,3): self.text(gx+xx*cell+10,gy+yy*cell+36,'Goal',14,w=44)
                    if (xx,yy)==(2,3): self.text(gx+xx*cell+10,gy+yy*cell+36,'Pawn',14,w=44)
            self.panel(x+544,y+184,w-544,h-196,'Draft status')
            self.rows(x+552,y+224,w-560,['Source|G1 board','State|Local draft','Apply|One validated Undo','Stale source|Discard before Apply'],80)
            return
        if template=='merge':
            self.toolbar(x,y,w,e['actions'])
            self.text(x,y+68,'Conflict: /game/scenes/arena/objects · Whole-array conflict is possible',14,C['warning'],w=w)
            cw=(w-24)/3
            for i,title in enumerate(['Base','Current','Other']):
                self.code_box(x+i*(cw+12),y+92,cw,h-180,title,'[\n  { "id": 10,\n    "...": "exact payload" }\n]')
            self.text(x,y+h-52,'Choose Keep current / Take other / Use base; validate all conflicts before Apply.',14,w=w)
            return
        if template=='hierarchy':
            self.rect(x,y,440,32,C['control'],C['boundary'],4)
            self.text(x+12,y+22,'Arena 01',14,w=356)
            self.icon('chevron',x+368,y+6)
            self.button(x+400,y,'…',w=40)
            self.rect(x,y+40,400,28,C['canvas'],C['boundary'],4)
            self.text(x+8,y+60,'Search objects',14,C['secondary'],w=380)
            self.button(x+408,y+38,'+',w=32)
            self.text(x,y+92,'27 objects · Entry scene',12,C['secondary'],w=440)
            for i,label in enumerate(['Hero','Props','Environment','  Floor','  Wall','Object 1000']):
                yy=y+108+i*28
                if i==0:self.rect(x,yy,440,28,C['selected']);self.rect(x,yy,2,28,C['accent'])
                if i in (1,2):self.icon('chevron_right' if i==1 else 'chevron',x+4,yy+4)
                self.text(x+28,yy+20,label,14,w=400)
            self.text(x+480,y+24,'Hierarchy interaction reference',16,w=w-480,weight=600)
            for i,line in enumerate(['Click selects the Scene / Inspector object.','F2 or context menu renames; Escape cancels.','Delete belongs to the focused object row.','Search reveals matches and their parents.','Scene file commands live in … / File / Assets.','IDs and component keys are available on hover.']):
                self.text(x+480,y+64+i*48,line,14,C['secondary'],w=w-480)
            return
        if template=='tree':
            self.toolbar(x,y,w,e['actions'])
            offset=48
            self.panel(x,y+offset,440,h-offset-12,'Legacy runtime tree')
            for i,row in enumerate(rows):
                yy=y+offset+56+i*48;label=row.split('|')[0]
                if i==0:self.rect(x+4,yy-20,432,28,C['selected'])
                if label in ('Monsters','Environment'):self.icon('chevron_right' if label=='Monsters' else 'chevron',x+12,yy-14)
                self.text(x+40,yy+2,label+(' · selected' if i==0 else ''),14,w=388)
            self.panel(x+456,y+offset,w-456,h-offset-12,'Selection and context')
            self.text(x+472,y+offset+60,'The selected identity belongs to this view’s data scope.',14,w=w-488)
            self.text(x+472,y+offset+128,'Legacy selection is separate from authored Hierarchy.',14,C['secondary'],w=w-488)
            return
        if template=='help':
            self.text(x,y+20,'Icon samples · not an editor control bar',14,C['secondary'],w=w)
            for i,name in enumerate(['play','stop','save','move','rotate','scale']): self.button(x+i*44,y+32,'',32,icon=name,selected=name=='move')
            for i,(title,copy) in enumerate([('Save Project · Ctrl+S','Save authored changes and supported drafts.'),('Depth texture · input','Show actual type, format, extent and samples.'),('Unavailable Play','Preparing game logic. The same help is reachable on keyboard focus.')]):
                tx=x+i*380; self.rect(tx,y+96,320,168,C['raised'],C['boundary'],4)
                self.text(tx+8,y+124,title,14,w=304,weight=600)
                self.text(tx+8,y+156,copy,14,C['secondary'],w=304)
            self.text(x,y+324,'Critical errors stay visible outside hover help. Escape dismisses temporary help.',14,C['warning'],w=w)
            return
        if template=='controls':
            self.widgets(x,y,w*.62,e['layout'].get('widgets',[]))
            px=x+w*.65; pw=w*.35
            self.panel(px,y,pw,304,'Stored colour (example)')
            self.rect(px+12,y+48,pw-24,72,C['accent'],C['boundary'],4)
            self.rows(px+12,y+136,pw-24,['R / G / B / A|Stored channel values','Renderer support|Separate contract'],72)
            self.text(x,y+340,'Tool icon samples · not scene commands',12,C['secondary'],w=w)
            self.toolbar(x,y+352,w,['Move','Rotate','Scale'])
            self.rect(x,y+404,w,168,C['raised'],C['boundary'],4)
            self.text(x+12,y+432,'Normal · hover · pressed · focused · checked · disabled',14,w=w-24)
            for i,label in enumerate(['Normal','Hover','Pressed','Focus','Checked','Disabled']):
                bx=x+20+i*180; by=y+452
                self.button(bx,by,'',32,icon='move',selected=label=='Checked',disabled=label=='Disabled')
                if label=='Hover':
                    self.rect(bx,by,32,32,C['hover'],C['boundary'],4);self.icon('move',bx+6,by+6)
                if label=='Pressed':
                    self.rect(bx,by,32,32,C['selected'],C['boundary'],4);self.icon('move',bx+7,by+7)
                if label=='Focus':self.rect(bx-2,by-2,36,36,'none',C['accent'],4,2)
                self.text(bx,by+58,label,12,C['secondary'],w=160)
            self.text(x+12,y+548,'Visible focus/selection and labels supplement colour. A colour control does not add alpha blending.',12,C['secondary'],w=w-24)
            return
        if template=='extension':
            self.rows(x,y,w,rows[:1],72)
            self.panel(x,y+92,w,h-200,'Module content slot / no registered implementation in this contract example')
            self.text(x+16,y+160,'The owning module supplies its real controls. This contract frame does not invent a working domain tool.',14,C['secondary'],w=w-32)
            self.button(x,y+h-84,'Apply draft',disabled=True)
            self.text(x+180,y+h-64,'Conditional footer only when a real stateful tool provides a candidate.',12,C['secondary'],w=w-196)
            return
        self.toolbar(x,y,w,e['actions'] if self.entry['group']!='components' else [])
        if e['layout'].get('widgets'):self.widgets(x,y+56,w,e['layout']['widgets'])
        else:self.rows(x,y+56,w,rows,80 if len(rows)<6 else 76)
        if template in ('fields','references','controls'):
            self.rect(x,y+h-92,w,80,C['raised'],C['boundary'],4)
            self.text(x+12,y+h-64,'Authored / effective / inherited / unknown are distinct states.',14,w=w-24)
            self.text(x+12,y+h-36,'No-op Apply preserves inheritance. Invalid input preserves the previous exact value.',12,C['secondary'],w=w-24)

    def code_box(self,x,y,w,h,title,code):
        self.panel(x,y,w,h,title)
        yy=y+60
        for i,raw in enumerate(code.split('\n')):
            self.text(x+8,yy,str(i+1),12,C['secondary'],w=28)
            yy=self.text(x+48,yy,raw,13,w=w-64,code=True)

    def menu_body(self,x,y,w,h):
        e=self.entry
        rows=e['layout']['rows']
        columns=[('Context menu',rows)]
        if e['id']=='M01':
            columns=[('File',['New Project >','Open Project…|Ctrl+O','Recent Projects >','Save Project|Ctrl+S','Save As…|Ctrl+Shift+S','Import Asset…','Export >','Exit']),
             ('New Project',['Inherit current Engine','Choose permitted composition','Empty scene','Two arenas example','Crate puzzle example','Crate Garden (G1)']),
             ('Recent Projects',['project.json|Full path on help','No recent projects|Empty-list variant']),
             ('Export',['Windows Player…','Team editor…','Full source project…'])]
        if e['id']=='M06':
            self.panel(x+720,y,w-720,220,'Join preview')
            self.rect(x+732,y+48,210,148,C['selected'],C['accent'],4)
            self.text(x+744,y+80,'Keep this area',14,w=186)
            self.rect(x+952,y+48,w-964,148,C['raised'],C['boundary'],4)
            self.text(x+964,y+80,'Join neighbor',12,w=w-988)
        cw=min(336,(w-12*(len(columns)-1))/len(columns))
        for i,(title,items) in enumerate(columns):
            tx=x+i*(cw+12)
            heights=[]
            for item in items:
                cells=item.split('|')
                line_count=len(wrap(cells[0],cw-24))
                if len(cells)>1:line_count+=len(wrap(' / '.join(cells[1:]),cw-24,12))
                heights.append(max(28,8+20*line_count))
            self.panel(tx,y,cw,40+sum(heights),title)
            yy=y+40
            for item,rh in zip(items,heights):
                cells=item.split('|')
                bottom=self.text(tx+12,yy+18,cells[0],14,w=cw-24)
                if len(cells)>1:self.text(tx+12,bottom,' / '.join(cells[1:]),12,C['secondary'],w=cw-24)
                yy+=rh
        if len(columns)==1 and e['id']!='M06':
            self.panel(x+cw+24,y,w-cw-24,h,'Command context')
            self.text(x+cw+40,y+64,e['entry']['en'],14,w=w-cw-56)
            self.text(x+cw+40,y+128,e['requirements'][0]['en'],14,C['secondary'],w=w-cw-56)

    def dialog_body(self,x,y,w,h):
        e=self.entry; native=e['group']=='native'; template=e['layout']['template']
        dw=960 if template=='native_picker' else 720 if template in ('report','asset_delete','preset_review') or e['id']=='D01' else 560
        dh=540 if template=='native_picker' else 480
        dx=x+(w-dw)/2; dy=y+32
        self.rect(x,y,w,h,C['canvas'])
        self.panel(dx,dy,dw,dh,'OS-owned schematic' if native else e['label']['en'])
        if template=='native_picker':
            self.text(dx+12,dy+60,'Application supplies title / current folder hint / filters',14,C['secondary'],w=dw-24)
            self.panel(dx+12,dy+80,200,dh-160,'Locations (OS)')
            self.rows(dx+20,dy+120,184,['Current project','Parent folder','Available locations'],64)
            self.rows(dx+228,dy+88,dw-240,e['layout']['rows'],80)
        elif e['id']=='D01':
            self.rows(dx+12,dy+44,dw-24,e['layout']['rows'][:2],64)
            self.rect(dx+12,dy+184,dw-24,112,C['raised'],C['boundary'],4)
            self.text(dx+24,dy+208,'Unresolved module draft · Crate Garden Level Palette',14,w=dw-48)
            self.text(dx+24,dy+236,'Apply in the owning tool or explicitly discard only this draft.',12,C['secondary'],w=dw-48)
            self.button(dx+24,dy+252,'Discard module draft')
            self.text(dx+12,dy+332,'Save and continue is unavailable until required drafts are resolved.',14,C['warning'],w=dw-24)
            self.text(dx+12,dy+368,'Secondary: Use saved version (Check/export) / Don’t save (eligible transitions). Neither bypasses an asset transaction.',12,C['secondary'],w=dw-24)
        elif e['layout'].get('widgets'):
            self.widgets(dx+12,dy+44,dw-24,e['layout']['widgets'])
        else:
            self.rows(dx+12,dy+44,dw-24,e['layout']['rows'],72)
        self.line(dx+12,dy+dh-68,dx+dw-12,dy+dh-68,C['separator'])
        actions=e['actions']
        # Footer shows only actual confirm/close choices, not embedded form actions.
        if e['id']=='D01':actions=['Save and continue','Cancel']
        elif e['id']=='D02':actions=['Copy details','Close']
        elif e['id']=='D11':actions=['Apply replacement','Cancel']
        elif native: actions=['Confirm selection' if e['id']!='N08' else 'Confirm overwrite','Cancel']
        if e['id']=='D01':
            self.button(dx+12,dy+dh-52,'Save and continue',disabled=True)
            self.button(dx+224,dy+dh-52,'Cancel')
        else:self.toolbar(dx+12,dy+dh-52,dw-24,actions,use_icons=False)
        if native:
            self.text(x+12,y+h-28,'The OS controls actual appearance, language, DPI and focus. This is not a custom picker implementation.',12,C['secondary'],w=w-24)

    def states(self):
        for i,(key,copy) in enumerate(self.entry['states'].items()):
            x=16+(i%2)*632; y=80+(i//2)*220
            self.rect(x,y,616,204,C['panel'],C['boundary'],4)
            self.rect(x,y,616,32,C['raised'])
            self.text(x+12,y+22,DATA['state_names'][key]['en'],16,w=592,weight=600)
            value=copy['en']; na=value.startswith('NA:')
            if na:value='Not applicable — '+value[3:].strip()
            self.text(x+12,y+60,value,14,C['secondary'] if na else C['text'],w=592)
            self.text(x+12,y+186,'N/A is explicit, not an unimplemented control.' if na else 'Required design behavior · verify in a future implementation',12,C['secondary'],w=592)
        self.text(16,998,'State cards document local variants. They do not claim real commands, native behavior or performance.',12,C['secondary'],w=1248)

    def finish(self,state_sheet=False):
        if state_sheet:self.states()
        elif self.entry['layout'].get('refinement')=='hierarchy-v1':
            Hierarchy(self,C,TOKENS).draw(self.compact)
            self.text(16,886,'Proposed area layout · no native UI acceptance.' if self.compact else 'Hierarchy prototype · See editor-ui-hierarchy.md for grouping, emphasis and compact behavior.',12,C['secondary'],w=self.w-32)
        else:
            self.body(*self.editor_frame())
            self.text(16,886,'Logical dimensions follow editor-ui-spec.md. Data and controls are illustrative design requirements.',12,C['secondary'],w=1248)
        title=self.entry['id']+' / '+self.entry['label']['en']+(' / compact area' if self.compact else ' / states' if state_sheet else ' / layout')
        content=f'<svg xmlns="http://www.w3.org/2000/svg" width="{self.w}" height="{self.h}" viewBox="0 0 {self.w} {self.h}" role="img" aria-labelledby="title desc">\n<title id="title">{esc(title)}</title>\n<desc id="desc">Static ModuleEngine editor design proposal. Not a running editor screenshot. See the bilingual catalogue for requirements and actual implementation boundaries.</desc>\n'+ '\n'.join(self.parts)+'\n</svg>\n'
        return content

def relative_link(page, target):
    return os.path.relpath(ROOT/target,page.parent).replace('\\','/')

def requirements_page(group,lang):
    name='editor-ui-'+group+'.md'
    page=DOCS/('ru' if lang=='ru' else '')/name
    en,ru,_=GROUPS[group]
    title=ru if lang=='ru' else en
    counterpart='../'+name if lang=='ru' else 'ru/'+name
    nav=f'[English]({counterpart if lang=="ru" else name}) | [Русский]({name if lang=="ru" else counterpart}) | '+('[Реестр](editor-ui-catalog.md)' if lang=='ru' else '[Catalogue](editor-ui-catalog.md)')
    intro=('Полный смысл EN/RU берётся из одного двуязычного реестра. Все картинки — предлагаемый дизайн, не снимки работающего редактора. UI на макетах английский согласно языковому контракту. Восемь категорий состояний приведены для каждого элемента; N/A объяснено. Наличие нынешнего интерфейса не означает, что предложенная вёрстка реализована.' if lang=='ru' else
           'Complete EN/RU meaning comes from one bilingual catalogue. All pictures are design proposals, not running-editor captures. Mockup UI is English under the language contract. Eight state categories are specified per surface, including explained N/A. An existing surface does not mean its proposed layout is implemented.')
    out=[f'# {title}',nav,intro]
    for e in DATA['entries']:
        if group_for(e)!=group:continue
        id=e['id']; anchor=id.lower()
        out += [f'<a id="{anchor}"></a>',f'## {id} — {e["label"][lang]}',
                f'**{STATUS[e["status"]][lang=="ru"]}.**',
                ('**Назначение:** ' if lang=='ru' else '**Purpose:** ')+e['purpose'][lang],
                ('**Вход:** ' if lang=='ru' else '**Entry:** ')+e['entry'][lang],
                ('**Доступ и область:** ' if lang=='ru' else '**Access and scope:** ')+e['rights'][lang],
                ('**Исходники:** ' if lang=='ru' else '**Sources:** ')+', '.join(f'[{src}]({relative_link(page,src.split("::")[0])})' for src in e['sources']),
                '### '+('Требования' if lang=='ru' else 'Requirements')]
        out += ['- '+v[lang] for v in e['requirements']]
        out += [('**Действия / элементы:** ' if lang=='ru' else '**Actions / controls:** ')+(', '.join('`'+v+'`' for v in e['actions']) or ('только чтение' if lang=='ru' else 'read-only'))]
        if e['layout'].get('widgets'):
            out += ['### '+('Поля и привязки' if lang=='ru' else 'Fields and bindings'),
                    ('Тип widget описывает отображение, не новый тип сериализации. Значения примеров не являются правками пользовательского проекта; readonly не рисуется текстовым полем.' if lang=='ru' else 'Widget kind describes presentation, not a new serialization type. Example values are not edits to the user project; readonly is not drawn as a text input.'),
                    '| '+('Элемент UI | Вид | Пример | Контракт / привязка' if lang=='ru' else 'UI control | Kind | Example | Contract / binding')+' |','| --- | --- | --- | --- |']
            for item in e['layout']['widgets']:
                value=' / '.join(item['value']) if isinstance(item['value'],list) else str(item['value'])
                out.append('| '+item['label']+' | `'+item['kind']+'` | '+md(value)+' | '+md(item['rule'][lang])+' |')
        out += ['### '+('Состояния' if lang=='ru' else 'States'),
                '| '+('Состояние | Требуемое поведение' if lang=='ru' else 'State | Required behavior')+' |','| --- | --- |']
        for state,copy in e['states'].items():
            text=copy[lang]
            if text.startswith('NA:'):text=('Не применяется: ' if lang=='ru' else 'Not applicable: ')+text[3:].strip()
            out.append('| '+DATA['state_names'][state][lang]+' | '+md(text)+' |')
        out += [('**Клавиатура / фокус:** ' if lang=='ru' else '**Keyboard / focus:** ')+e['keyboard'][lang],
                ('**Данные / жизненный цикл:** ' if lang=='ru' else '**Data / lifecycle:** ')+e['persistence'][lang],
                '**'+('Проверка реализации:' if lang=='ru' else 'Implementation acceptance:')+'**']
        out += ['- '+v[lang] for v in e['acceptance']]
        media='../media/editor-ui-catalog/' if lang=='ru' else 'media/editor-ui-catalog/'
        variants=f'[{"Основной вид" if lang=="ru" else "Layout"}]({media}{id.lower()}.svg) · [{"Лист состояний" if lang=="ru" else "State sheet"}]({media}{id.lower()}-states.svg)'
        if e['layout'].get('compact_mockup'):
            variants+=f' · [{"Узкая область" if lang=="ru" else "Compact area"}]({media}{id.lower()}-compact.svg)'
            out += [('**Визуальная иерархия:** [правила и геометрия](editor-ui-hierarchy.md).' if lang=='ru' else '**Visual hierarchy:** [rationale and geometry](editor-ui-hierarchy.md).')]
        out += [('**Макеты:** ' if lang=='ru' else '**Mockups:** ')+variants+'.']
    return page,markdown_blocks(out)

def catalogue_page(lang):
    page=DOCS/('ru' if lang=='ru' else '')/'editor-ui-catalog.md'
    ru=lang=='ru'; prefix='../' if ru else ''
    out=['# '+('Полный каталог UI редактора' if ru else 'Complete editor UI catalogue'),
         f'[English]({"../" if ru else ""}editor-ui-catalog.md) | [Русский]({"" if ru else "ru/"}editor-ui-catalog.md) | '+('[Документация](README.md)' if ru else '[Documentation](README.md)'),
         ('**Дизайн-предложение от 14 ноября 2026; реализация редактора не менялась.** Это ограниченный текущими исходниками реестр '+str(len(DATA['entries']))+' элементов: 19 встроенных типов окон, 2 настоящие панели модулей и контракт будущих расширений, 13 вложенных/планируемых видов, 2 элемента каркаса, 7 существующих целей модальных окон приложения и 5 предлагаемых подтверждений/форм, 7 целей системных picker и отдельное подтверждение перезаписи ОС, 11 меню и 4 семейства общих элементов.' if ru else
          '**Design proposal, 14 November 2026; editor implementation unchanged.** This source-bounded catalogue contains '+str(len(DATA['entries']))+' surfaces: 19 builtin area types, 2 real module panels and a future-extension contract, 13 nested/planned views, 2 shell surfaces, 7 existing app-modal purposes and 5 proposed confirmations/forms, 7 native picker purposes and a separate OS overwrite confirmation, 11 menus and 4 shared-control families.'),
         ('«Полный» означает покрытие перечисленных нынешних production-регистраций/целей и уже запрошенных направлений UI. Это не все возможные сочетания данных и не дизайн будущего неизвестного плагина. Новая регистрация требует своей записи/макета. Четыре разных контекста графа — root, GPU, Gameplay Systems, Rust — сохраняют разные данные, исполнение и права.' if ru else
          'Complete means coverage of the listed current production registrations/purposes and already requested UI directions. It is not every possible data combination or a bespoke design for an unknown future plugin. New registrations require a new entry/mockup. Root, GPU, Gameplay Systems and Rust graphs keep different data, execution and access contracts.'),
         ('[Атлас макетов](../media/editor-ui-catalog/index.html) позволяет выбрать каждый интерфейс и переключить основной вид/состояния. Это средство чтения документации, его кнопки не управляют движком.' if ru else
          '[Mockup atlas](media/editor-ui-catalog/index.html) lets you select every surface and switch layout/state sheets. It is a documentation viewer; its controls do not operate the engine.'),
         ('[Уточнение визуальной иерархии](editor-ui-hierarchy.md) дорабатывает P01 Engine Overview, P08 Inspector и P09 Assets: группировку, выбор, основные действия и три дополнительных SVG узкой области. В атласе они доступны через Compact area. Остальные компоновки этим уточнением не переработаны.' if ru else
          '[Visual hierarchy refinement](editor-ui-hierarchy.md) revises P01 Engine Overview, P08 Inspector and P09 Assets: grouping, selection, primary actions and three additional compact-area SVGs. Choose Compact area in the atlas. Other layouts are not redesigned by this refinement.'),
         '## '+('Правила и границы' if ru else 'Rules and boundaries'),
         ('[Дизайн](editor-ui-design.md) и [размеры/токены](editor-ui-spec.md) остаются общей базой. Игровой HUD, экраны меню самой игры, будущие G2-графы анимации/ИИ и управление отдельными движками не входят в нынешний UI редактора. В Game они могут быть содержимым проекта; шаблон P22 не навязывает игре бой, инвентарь или головоломку.' if ru else
          '[Design](editor-ui-design.md) and [metrics/tokens](editor-ui-spec.md) remain the shared baseline. Game HUD/menu screens, future G2 animation/AI graphs and independent-engine management are outside the current editor UI. Game may display project-owned content; P22 does not impose combat, inventory or puzzles on a game.'),
         ('Состояния: готово, пусто/нет выбора, занято, черновик, ошибка/отклонение, ограничение, отсутствующая/устаревшая зависимость, узкая область. Каждая запись содержит все восемь с конкретными уточнениями; неприменимость объясняется. Отдельно заданы фокус/Cancel и сохранение. Главное изображение показывает предлагаемую компоновку, лист состояний — локальные варианты/сообщения, а не восемь работающих сборок.' if ru else
          'States: ready, empty/no selection, busy, draft, error/rejection, restricted, missing/stale dependency and compact area. Every entry specifies all eight with contextual rules or an explained N/A. Focus/Cancel and persistence are separately defined. The primary image shows the proposed arrangement; state sheets show local variants/copy, not eight working builds.'),
         ('Exact JSON, null, неизвестные данные, stable IDs, author/Play, Full/Team, pins и запреты отсутствующих модулей сохраняются. Ошибка публикации может оставить разрешённые контрактом recovery-файлы: нужно указать их пути, а не обещать отсутствие любых следов записи. Сохранено не равно применено. Поле read-only/Unknown не заменяется выдуманным default.' if ru else
          'Exact JSON, null, unknown payloads, stable IDs, authored/Play separation, Full/Team, pins and absent-module restrictions survive. Publication errors may leave contract-permitted recovery files: identify their paths instead of promising no filesystem traces. Saved is not applied. Read-only/Unknown is not replaced with a fabricated default.'),
         '## '+('Уточнённые предлагаемые решения' if ru else 'Consolidated proposed choices'),
         ('Это конкретные варианты для оценки дизайна, не утверждение согласования новой реализации пользователем:' if ru else
          'These are concrete choices for design review, not a claim that the user accepted a new implementation:'),
         ('- D08: + копирует текущую раскладку после ввода имени; доступные пресеты — альтернативы. D09/D10 работают с личными раскладками; D12 явно сбрасывает выбранную.\n- S02: перенос области, не дублирование; Close возвращает её в главное окно, не закрывает проект.\n- P09: Show address открывает выделяемый read-only путь относительно проекта; Copy absolute path отдельно, без ручного перехода по вводу.\n- P17–P19 остаются legacy-маршрутом со стабильными ID; смена пресета идёт через D11.\n- Иконки — общая собственная векторная геометрия; overlays сначала имеют фиксированные разумные места/переполнение.' if ru else
          '- D08: + clones the current layout after naming; available presets are alternatives. D09/D10 affect custom layouts; D12 explicitly resets the selected arrangement.\n- S02: move an area, do not duplicate it; Close returns it to the main window, not closing the project.\n- P09: Show address exposes selectable read-only project-relative text; Copy absolute path is separate, with no manual path navigation.\n- P17–P19 remain legacy routes with stable IDs; preset replacement goes through D11.\n- Icons use shared project-owned vector geometry; overlays start with fixed sensible anchors/overflow.'),
         '## '+('Размеры макетов' if ru else 'Mockup geometry'),
         ('Основные SVG: 1280×900 логических единиц листа; листы состояний 1280×1024. Внешняя подпись — аннотация документа, не часть продукта. Общая панель 32 + проектная 36 + статус 24; заголовок области 32. Строки UI/меню 28, toolbar-иконка 20 в зоне 32, Scene 20 в зоне 36. Стандартная app-форма 560, расширенная 720; поля растут/переносятся, body прокручивается, footer остаётся доступен. Общий menu-контейнер до 336, строки не обрезают двухстрочную подпись. Для нативных picker размеры схемы не предписывают ОС реальную геометрию. Адаптация к узкому экрану описана в каждой записи и общей спецификации.' if ru else
          'Primary SVGs: 1280×900 sheet logical units; state sheets 1280×1024. The outer caption is a documentation annotation, not product UI. Topbar 32 + projectbar 36 + status 24; area header 32. UI/menu rows 28, toolbar glyph 20 in target 32, Scene glyph 20 in target 36. Standard app form width 560, wide form 720; fields wrap/grow, body scrolls and footer remains reachable. Menu container up to 336; rows never clip wrapped labels. Native picker schematic dimensions do not prescribe actual OS geometry. Narrow-screen behavior is specified per entry and in the shared spec.'),
         '## '+('Реестр и трассировка' if ru else 'Inventory and traceability'),
         '| ID | '+('Интерфейс | Статус | Требования | Макет / состояния' if ru else 'Surface | Status | Requirements | Layout / states')+' |','| --- | --- | --- | --- | --- |']
    for e in DATA['entries']:
        id=e['id'].lower(); group=group_for(e); image=prefix+'media/editor-ui-catalog/'+id
        compact=f' / [{"Узкий" if ru else "Compact"}]({image}-compact.svg)' if e['layout'].get('compact_mockup') else ''
        out.append(f'| {e["id"]} | {e["label"][lang]} | {STATUS[e["status"]][ru]} | [→](editor-ui-{group}.md#{id}) | [SVG]({image}.svg) / [SVG]({image}-states.svg){compact} |')
    out += ['## '+('Проверка и воспроизведение' if ru else 'Verification and reproduction'),
            ('Канонические двуязычные данные — [catalog.json](../ui-catalog/catalog.json), генератор — [generate.py](../ui-catalog/generate.py). Генерация не меняет редактор, проекты или личные настройки. Требуется только стандартная библиотека Python 3.9+.' if ru else
             'Canonical bilingual data: [catalog.json](ui-catalog/catalog.json); generator: [generate.py](ui-catalog/generate.py). Generation does not modify the editor, projects or personal settings. Only Python 3.9+ standard library is required.'),
            '```text\npython docs/ui-catalog/generate.py\npython docs/ui-catalog/generate.py --check\n```',
            ('Проверка охватывает сопоставление source-регистраций/диалогов с реестром, EN/RU/состояния, наличие/согласованность всех макетов, ссылки, геометрию и контраст. QA требует отрисовки всех SVG и осмотра каждого листа; тест атласа проверяет выбор/поиск/состояния, а не команды движка. Итоговое независимое заключение и привязка источников — в `outputs/reviews/editor-ui-catalog-20261114/`. Скриншоты не доказывают нативный ввод, DPI, доступность ОС, сохранение, FPS или выполнение требований в продукте.' if ru else
             'Verification covers source registration/dialog mapping, EN/RU/state coverage, presence/consistency of every mockup, links, geometry and contrast. QA requires rasterizing all SVGs and inspecting every sheet; atlas checks test selection/search/state navigation, not engine commands. Final independent evidence/source binding lives in `outputs/reviews/editor-ui-catalog-20261114/`. Pictures do not prove native input, DPI, OS accessibility, persistence, FPS or implementation of these requirements.')]
    return page,markdown_blocks(out)

def atlas():
    entries=[dict(id=e['id'], label=e['label'], group=group_for(e),status=e['status'],purpose=e['purpose'],rights=e['rights'],requirements=e['requirements'],states=e['states'],keyboard=e['keyboard'],persistence=e['persistence'],widgets=e['layout'].get('widgets',[]),compact=bool(e['layout'].get('compact_mockup'))) for e in DATA['entries']]
    payload=json.dumps(entries,ensure_ascii=False).replace('<','\\u003c')
    return '''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>ModuleEngine UI catalogue — design proposals</title>
<style>
:root{color-scheme:dark;font:14px/1.5 "Segoe UI",Arial,sans-serif;background:#111823;color:#E6EDF5}*{box-sizing:border-box}body{margin:0}button,input,select{font:inherit;color:inherit;background:#28394D;border:1px solid #91A1B8;border-radius:4px;padding:6px 10px}button{cursor:pointer}button:focus-visible,input:focus-visible,select:focus-visible,a:focus-visible{outline:2px solid #66D6C4;outline-offset:2px}button[aria-pressed=true]{background:#234D50;border-color:#66D6C4}header{padding:16px 20px;border-bottom:1px solid #334155}h1{font-size:20px;margin:0}header p{color:#B6C4D5;margin:4px 0 12px}.controls{display:flex;gap:8px;flex-wrap:wrap;align-items:center}.controls label{display:flex;gap:8px;align-items:center}.layout{display:grid;grid-template-columns:300px minmax(0,1fr);min-height:80vh}nav{padding:12px;border-right:1px solid #334155;max-height:calc(100vh - 170px);overflow:auto}nav button{display:block;text-align:left;width:100%;margin-bottom:6px}main{padding:16px;min-width:0}h2{font-size:18px;margin:0 0 8px}#purpose{max-width:90ch}#preview{display:block;width:100%;height:auto;background:#192230;margin-top:12px}a{color:#66D6C4}.meta{color:#B6C4D5}#count{margin-left:auto}.mode{display:flex;gap:8px}#empty{padding:24px;border:1px solid #91A1B8}[hidden]{display:none!important}@media(max-width:760px){.layout{grid-template-columns:1fr}nav{max-height:240px;border-right:0;border-bottom:1px solid #334155}header{padding:12px}.controls label{width:100%}.controls input,.controls select{flex:1;min-width:0}#count{margin-left:0}}
</style><style>details{margin:12px 0;padding:12px;background:#192230;border:1px solid #334155;border-radius:4px}summary{cursor:pointer;font-weight:600}#spec{overflow-wrap:anywhere}#spec table{border-collapse:collapse;table-layout:fixed;width:100%;margin-top:12px}#spec td,#spec th{padding:8px;text-align:left;border-bottom:1px solid #334155;vertical-align:top}#spec th{width:160px}#spec li{margin:6px 0}main{overflow-wrap:anywhere}</style></head><body>
<header><h1>ModuleEngine / UI catalogue</h1><p id="notice">Design proposals and required states. This viewer does not run engine commands.</p>
<div class="controls"><label for="search">Search <input id="search" type="search" autocomplete="off"></label>
<label for="group">Group <select id="group"><option value="all">All</option><option value="windows">Windows</option><option value="views">Views / controls</option><option value="dialogs">Dialogs</option><option value="menus">Menus</option></select></label>
<label for="lang">Language <select id="lang"><option value="en">English</option><option value="ru">Русский</option></select></label><span id="count" aria-live="polite"></span></div></header>
<div class="layout"><nav id="surfaces" aria-label="UI surfaces"></nav><main><h2 id="label"></h2><p id="status" class="meta"></p><p id="purpose"></p>
<div class="controls"><div class="mode" aria-label="Mockup type"><button id="layout" type="button" aria-pressed="true">Layout</button><button id="states" type="button" aria-pressed="false">States</button><button id="compact" type="button" aria-pressed="false" hidden>Compact area</button></div><a id="requirements">Requirements</a><a id="fullsize" target="_blank" rel="noopener">Full-size SVG</a></div>
<details id="specbox"><summary id="specsummary">Requirements and states</summary><div id="spec"></div></details><p id="empty" hidden>No matching surfaces. Clear search or change the group.</p><img id="preview" alt="Selected ModuleEngine UI design proposal"></main></div>
<script>
const entries=CATALOG_DATA;
const byId=Object.fromEntries(entries.map(e=>[e.id,e]));
const el=id=>document.getElementById(id);
let selected='P01',mode='layout';
const names={current_surface:{en:'Current surface; proposed layout',ru:'Существующий интерфейс; предлагаемая вёрстка'},proposed_surface:{en:'Proposed surface; not implemented',ru:'Предложение; не реализовано'},extension_contract:{en:'Future extension contract',ru:'Контракт будущего расширения'},shared_component:{en:'Shared control family',ru:'Общие элементы'}};
function render(){
 const lang=el('lang').value,q=el('search').value.toLocaleLowerCase(),group=el('group').value;
 const shown=entries.filter(e=>(group==='all'||e.group===group)&&(e.id+' '+e.label.en+' '+e.label.ru).toLocaleLowerCase().includes(q));
 if(!shown.some(e=>e.id===selected))selected=shown[0]?.id||null;
 el('surfaces').replaceChildren();
 for(const e of shown){const b=document.createElement('button');b.type='button';b.textContent=e.id+' · '+e.label[lang];b.dataset.surface=e.id;b.setAttribute('aria-pressed',String(e.id===selected));b.onclick=()=>{selected=e.id;render();};el('surfaces').append(b);}
 el('count').textContent=shown.length+' / '+entries.length;
 if(mode==='compact'&&(!selected||!byId[selected].compact))mode='layout';
 el('layout').setAttribute('aria-pressed',String(mode==='layout'));el('states').setAttribute('aria-pressed',String(mode==='states'));el('compact').setAttribute('aria-pressed',String(mode==='compact'));el('compact').hidden=!selected||!byId[selected].compact;
 el('preview').hidden=!selected;el('empty').hidden=!!selected;el('requirements').hidden=!selected;el('fullsize').hidden=!selected;el('specbox').hidden=!selected;
 el('label').textContent=selected?selected+' · '+byId[selected].label[lang]:'';el('status').textContent=selected?names[byId[selected].status][lang]:'';el('purpose').textContent=selected?byId[selected].purpose[lang]:'';
 if(selected){const e=byId[selected],id=selected.toLowerCase(),file=id+(mode==='states'?'-states':mode==='compact'?'-compact':'')+'.svg';el('preview').src=file;el('preview').alt=e.label[lang]+' — '+mode+' — design proposal';el('requirements').href='../../'+(lang==='ru'?'ru/':'')+'editor-ui-'+e.group+'.md#'+id;el('fullsize').href=file;
  el('specsummary').textContent=lang==='ru'?'Требования и состояния':'Requirements and states';el('spec').replaceChildren();
  const p=document.createElement('p');p.textContent=e.rights[lang];el('spec').append(p);const ul=document.createElement('ul');for(const r of e.requirements){const li=document.createElement('li');li.textContent=r[lang];ul.append(li);}el('spec').append(ul);
  if(e.widgets.length){const h=document.createElement('h3');h.textContent=lang==='ru'?'Поля и привязки':'Fields and bindings';el('spec').append(h);const dl=document.createElement('dl');for(const w of e.widgets){const dt=document.createElement('dt'),dd=document.createElement('dd');dt.textContent=w.label+' · '+w.kind+' · '+(Array.isArray(w.value)?w.value.join(' / '):String(w.value));dd.textContent=w.rule[lang];dl.append(dt,dd);}el('spec').append(dl);}
  const table=document.createElement('table');const body=document.createElement('tbody');for(const [key,copy] of Object.entries(e.states)){const tr=document.createElement('tr'),th=document.createElement('th'),td=document.createElement('td');th.scope='row';th.textContent=key;td.textContent=copy[lang];tr.append(th,td);body.append(tr);}table.append(body);el('spec').append(table);
  for(const value of [e.keyboard,e.persistence]){const para=document.createElement('p');para.textContent=value[lang];el('spec').append(para);}
 }
 document.documentElement.lang=lang;
}
el('search').oninput=render;el('group').onchange=render;el('lang').onchange=render;
el('layout').onclick=()=>{mode='layout';render();};el('states').onclick=()=>{mode='states';render();};el('compact').onclick=()=>{mode='compact';render();};render();
</script></body></html>
'''.replace('CATALOG_DATA',payload)

def generated():
    files={}
    manifest=[]
    for entry in DATA['entries']:
        for state in (False,True):
            svg=SVG(entry,state)
            text=svg.finish(state)
            name=entry['id'].lower()+('-states' if state else '')+'.svg'
            files[MEDIA/name]=text
            manifest.append(dict(id=entry['id'],kind='states' if state else 'layout',file=name,
                                 width=svg.w,height=svg.h,text_nodes=len(svg.boxes),sha256=hashlib.sha256(text.encode()).hexdigest()))
        if entry['layout'].get('compact_mockup'):
            svg=SVG(entry,compact=True);text=svg.finish();name=entry['id'].lower()+'-compact.svg'
            files[MEDIA/name]=text
            manifest.append(dict(id=entry['id'],kind='compact',file=name,width=svg.w,height=svg.h,text_nodes=len(svg.boxes),sha256=hashlib.sha256(text.encode()).hexdigest()))
    for lang in ('en','ru'):
        for group in GROUPS:
            p,text=requirements_page(group,lang);files[p]=text
        p,text=catalogue_page(lang);files[p]=text
    files[MEDIA/'index.html']=atlas()
    files[DOCS/'ui-catalog/artifacts.json']=json.dumps(dict(schema_version=1,surfaces=len(DATA['entries']),artifacts=manifest),indent=2)+'\n'
    return files

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true',help='Compare generated bytes without writing')
    args=parser.parse_args()
    ids=[e['id'] for e in DATA['entries']]
    if len(ids)!=len(set(ids)) or any(not re.fullmatch(r'[PVSDNMC]\d{2}',i) for i in ids):
        raise SystemExit('Invalid/duplicate surface IDs')
    files=generated()
    failures=[]
    for path,content in files.items():
        raw=content.encode('utf-8')
        if args.check:
            if not path.is_file() or path.read_bytes()!=raw:failures.append(str(path.relative_to(ROOT)))
        else:
            path.parent.mkdir(parents=True,exist_ok=True)
            path.write_bytes(raw)
    if failures:
        print(json.dumps(dict(result='fail',mismatches=failures),indent=2));return 1
    print(json.dumps(dict(result='pass',mode='check' if args.check else 'generate',surfaces=len(ids),files=len(files))))
    return 0

if __name__=='__main__':
    raise SystemExit(main())
