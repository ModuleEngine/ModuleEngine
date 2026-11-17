"""Focused, static hierarchy prototypes for P01/P08/P09; no editor integration.

Uses the existing SVG primitives and shared documentation tokens. Additional
glyphs are project-owned vector paths on the same 24-unit grid as the catalogue.
"""

ICONS = {
    'h_back': '<path d="M20 12 H4 M11 5 L4 12 L11 19"/>',
    'h_search': '<circle cx="10" cy="10" r="6"/><path d="M15 15 L21 21"/>',
    'h_check': '<path d="M4 12 L9 17 L20 6"/>',
    'h_file': '<path d="M5 2 H14 L20 8 V22 H5 Z M14 2 V8 H20"/>',
    'h_image': '<rect x="2" y="3" width="20" height="18" rx="2"/><circle cx="8" cy="9" r="2"/><path d="M3 19 L10 12 L15 17 L18 14 L22 18"/>',
    'h_audio': '<path d="M9 18 V5 L20 3 V16 M9 9 L20 7"/><ellipse cx="6" cy="18" rx="3" ry="2"/><ellipse cx="17" cy="16" rx="3" ry="2"/>',
}


class Hierarchy:
    def __init__(self, svg, colors, tokens):
        self.s, self.c, self.t = svg, colors, tokens['hierarchy']

    def text(self, x, y, value, width, size=14, muted=False, weight=400):
        return self.s.text(x,y,value,size,self.c['secondary'] if muted else None,w=width,weight=weight)

    def primary(self, x, y, label, width):
        self.s.parts.append('<g data-ui-role="primary-action">')
        self.s.rect(x,y,width,28,self.c['accent'],r=4)
        self.s.text(x+12,y+19,label,14,self.c['on_accent'],w=width-24,weight=600)
        self.s.parts.append('</g>')

    def quiet(self, x, y, label, width, icon=None):
        if icon:
            self.s.icon(icon,x,y+4,20)
        self.text(x+(28 if icon else 8),y+19,label,width-(36 if icon else 16))

    def field(self,x,y,w,value,enum=False,disabled=False,placeholder=False):
        self.s.rect(x,y,w,28,self.c['panel'] if disabled else self.c['control'],self.c['separator'] if disabled else self.c['boundary'],4)
        self.s.text(x+8,y+19,value,14,self.c['disabled'] if disabled else self.c['secondary'] if placeholder else None,w=w-(40 if enum else 16))
        if enum:self.s.icon('chevron',x+w-26,y+4,20,self.c['disabled'] if disabled else None)

    def section(self,x,y,w,label,disclosure=False):
        self.s.parts.append('<g data-ui-role="section-heading">')
        self.text(x+(24 if disclosure else 0),y+20,label,w-32,16,weight=600)
        if disclosure:self.s.icon('chevron',x,y+4,16)
        self.s.parts.append('</g>')

    def fact(self,x,y,w,label,value,label_width=152):
        self.text(x,y+19,label,label_width-12,12,muted=True)
        self.text(x+label_width,y+19,value,w-label_width)

    def frame(self,compact):
        s,c=self.s,self.c
        if compact:
            s.rect(16,72,448,792,c['panel'],c['separator'],4)
            s.rect(16,72,448,32,c['raised'])
            self.text(28,93,s.entry['label']['en'],392,weight=600)
            s.icon('chevron',432,78,20)
            self.text(28,128,'Example project · Full source',424,12,muted=True)
            s.rect(16,840,448,24,c['raised'])
            self.text(28,857,'Area prototype · illustrative data',424,12,muted=True)
            return 28,148,424,676
        s.rect(16,72,1248,792,c['panel'],c['separator'],4)
        s.rect(16,72,1248,32,c['raised'])
        for x,label in [(28,'File'),(68,'Edit'),(108,'Tools'),(160,'Windows')]:self.text(x,93,label,88)
        for x,label in [(336,'Engine'),(432,'Project'),(528,'My layout')]:
            active=label==('Engine' if s.entry['id']=='P01' else 'Project')
            if active:
                s.rect(x,74,88,28,c['selected'],r=4)
                s.rect(x+8,100,72,2,c['accent'])
            self.text(x+12,93,label,100 if label=='My layout' else 64,weight=600 if active else 400)
        self.text(656,94,'+',28,20)
        self.text(28,128,'Example project',192,weight=600)
        self.text(228,128,'Full · matching source installation',260,12,muted=True)
        s.button(500,106,'',32,icon='play')
        s.button(540,106,'',32,icon='stop',disabled=True)
        s.line(584,112,584,132)
        s.button(596,106,'',32,icon='save')
        self.text(1068,128,'Authoring scene',168,12,muted=True)
        s.rect(16,140,1248,32,c['raised'])
        self.text(28,161,s.entry['label']['en'],1168,weight=600)
        s.icon('chevron',1232,146,20)
        s.rect(16,840,1248,24,c['raised'])
        self.text(28,857,'Design proposal · example data',720,12,muted=True)
        return 28,184,1224,640

    def draw(self,compact=False):
        x,y,w,h=self.frame(compact)
        if self.s.entry['id']=='P01':self.overview(x,y,w,h,compact)
        elif self.s.entry['id']=='P08':self.inspector(x,y,w,h,compact)
        else:self.assets(x,y,w,h,compact)

    def overview(self,x,y,w,h,compact):
        s,c=self.s,self.c
        if compact:
            self.quiet(x,y,'Back to composition',240,icon='h_back')
            dx,dy,dw=x,y+48,w
        else:
            nw=self.t['overview_navigation'];lw=self.t['overview_list'];gap=self.t['group_gap']
            s.rect(x,y,nw,h,c['canvas'])
            self.text(x+12,y+28,'COMPOSITION',nw-24,12,muted=True,weight=600)
            for i,label in enumerate(['Technical Engine','Gameplay','Project','Unknown']):
                yy=y+48+i*self.t['navigation_row']
                if i==0:
                    s.rect(x+4,yy+4,3,28,c['accent'])
                self.text(x+16,yy+23,label,nw-28,weight=600 if i==0 else 400,muted=i!=0)
            lx=x+nw+gap
            self.section(lx,y,lw,'Subsystems')
            self.field(lx,y+36,lw,'Search composition',placeholder=True)
            for i,(label,note) in enumerate([('Rendering','Image and render passes'),('World storage','Entities and components'),('Input','Game input capability'),('Audio','Playback service')]):
                yy=y+80+i*self.t['subsystem_row']
                if i==0:
                    s.rect(lx,yy,lw,52,c['selected'],r=4)
                    s.rect(lx,yy+8,3,36,c['accent'])
                    s.icon('h_check',lx+lw-28,yy+14,16,c['accent'])
                self.text(lx+12,yy+21,label,lw-52,weight=600 if i==0 else 400)
                self.text(lx+12,yy+41,note,lw-24,12,muted=True)
            dx=lx+lw+gap;dy=y;dw=x+w-dx
            s.line(dx-12,y,dx-12,y+h,c['separator'])
        self.text(dx,dy+18,'Technical Engine / Rendering',dw,12,muted=True)
        self.text(dx,dy+54,'Rendering',dw,20,weight=600)
        self.text(dx,dy+82,'Turns the scene into the image shown on screen.',dw,muted=True)
        self.section(dx,dy+116,dw,'Current configuration')
        for i,(label,value) in enumerate([('Implementation','Vulkan renderer'),('Rendering path','Forward'),('Scene input','Scene3D')]):
            self.fact(dx,dy+152+i*self.t['metadata_row'],dw,label,value,144 if compact else 152)
        self.primary(dx,dy+260,'Open render graph',188)
        self.quiet(dx+(0 if compact else 204),dy+(300 if compact else 260),'Show renderer node',208)
        ey=dy+(360 if compact else 332)
        self.section(dx,ey,dw,'Capability evidence')
        for i,(label,value) in enumerate([('Installed','Yes · source registry'),('Offered','Yes · selected profile'),('Participating','Yes · authored route')]):
            self.fact(dx,ey+32+i*28,dw,label,value,144 if compact else 152)
        self.text(dx,ey+144,'These facts do not establish runtime readiness.',dw,12,muted=True)
        self.quiet(dx,ey+180,'Source details',176,icon='chevron_right')
        self.quiet(dx+200,ey+180,'Refresh evidence',176)

    def vector(self,x,y,w,label,values):
        label_width=self.t['form_label'];field_width=min(144,(w-label_width-16)/3)
        self.text(x,y+19,label,label_width-12)
        for i,(axis,value) in enumerate(zip('XYZ',values)):
            fx=x+label_width+i*(field_width+8)
            self.s.rect(fx,y,field_width,28,self.c['control'],self.c['boundary'],4)
            self.text(fx+8,y+19,axis,16,12,muted=True)
            self.text(fx+28,y+19,value,field_width-36)

    def inspector(self,x,y,w,h,compact):
        s,c=self.s,self.c
        fw=min(w,self.t['form_max_width']);fx=x if compact else x+24
        s.icon('mesh',fx,y+4,24)
        self.text(fx+36,y+23,'Hero',fw-100,20,weight=600)
        self.text(fx+36,y+48,'Arena 01 / Hero · Object ID 1',fw-36,12,muted=True)
        s.icon('more',fx+fw-28,y+8,20)
        self.text(fx,y+83,'Prefab',100,12,muted=True)
        self.field(fx+112,y+64,min(276,fw-112),'Hero template',enum=True)
        self.section(fx,y+116,fw,'Transform',True)
        for i,(label,values) in enumerate([('Position',['0.00','0.00','5.00']),('Rotation',['0.00','0.00','0.00']),('Scale',['1.00','1.00','1.00'])]):
            self.vector(fx,y+148+i*self.t['field_row'],fw,label,values)
        self.text(fx+112,y+280,'Rotation in radians · inherited values',fw-112,12,muted=True)
        self.section(fx,y+304,fw,'Actor',True)
        s.icon('more',fx+fw-28,y+308,20)
        self.text(fx+24,y+348,'Gameplay component · local override',fw-24,12,muted=True)
        self.text(fx,y+379,'Speed',100)
        self.field(fx+112,y+360,144,'6.5')
        self.text(fx+112,y+408,'Draft · authored value 6.0',fw-112,12,muted=True)
        self.primary(fx+112,y+420,'Apply Actor fields',172)
        self.section(fx,y+480,fw,'Health',True)
        s.icon('more',fx+fw-28,y+484,20)
        self.text(fx,y+547,'Current',100)
        self.field(fx+112,y+528,144,'100')
        self.text(fx+112,y+576,'Inherited · no local override',fw-112,12,muted=True)
        self.quiet(fx,y+608,'Add component',184)
        if not compact:
            self.text(x+836,y+28,'AUTHORING SELECTION',352,12,muted=True,weight=600)
            self.text(x+836,y+68,'One object',352,16,weight=600)
            self.text(x+836,y+100,'Edits belong to the authored scene.',352,muted=True)
            self.text(x+836,y+160,'Component drafts',352,16,weight=600)
            self.text(x+836,y+192,'Actor has unapplied field edits.',352,muted=True)
            self.text(x+836,y+244,'Transform gestures use Undo; component fields use their own Apply.',320,12,muted=True)

    def preview(self,x,y,w,h):
        s,c=self.s,self.c
        s.rect(x,y,w,h,c['canvas'],r=4)
        cx=x+w/2;cy=y+h/2
        s.parts.append(f'<path d="M{cx} {cy-44} l48 24 v52 l-48 24 l-48 -24 v-52 Z M{cx-48} {cy-20} l48 24 l48 -24 M{cx} {cy+4} v52" fill="{c["raised"]}" stroke="{c["icon"]}" stroke-width="2"/>')
        self.text(x+12,y+h-12,'Model preview · schematic',w-24,12,muted=True)

    def asset_details(self,x,y,w,compact=False):
        self.text(x,y+21,'hero.glb',w,20 if compact else 16,weight=600)
        self.text(x,y+44,'Model · selected asset',w,12,muted=True)
        self.preview(x,y+64,w,172 if compact else 164)
        fy=y+(260 if compact else 256)
        self.text(x,fy,'Location',w,12,muted=True)
        self.text(x,fy+24,'assets/models/hero.glb',w)
        self.text(x,fy+76,'Asset reference',w,12,muted=True)
        self.text(x,fy+100,'asset://a-hero',w,12)
        self.text(x,fy+136,'Selectable reference',w,12,muted=True)
        self.s.line(x,fy+176,x+w,fy+176)
        self.quiet(x,fy+192,'Usages…',136)
        self.quiet(x,fy+232,'Asset actions…',200,icon='more')

    def assets(self,x,y,w,h,compact):
        s,c=self.s,self.c
        if compact:
            self.quiet(x,y,'Back to models',224,icon='h_back')
            s.rect(x,y+40,w,28,c['control'],c['boundary'],4)
            s.rect(x+8,y+44,104,20,c['selected'])
            self.text(x+8,y+59,'assets/models',w-32)
            self.text(x,y+96,'Read-only address · show breadcrumbs',w,12,muted=True)
            self.asset_details(x,y+124,w,True)
            return
        self.text(x,y+21,'Assets',80,weight=600)
        s.rect(x,y+30,48,2,c['accent'])
        self.quiet(x+88,y,'Trash (0)',112)
        self.quiet(x+820,y,'New folder…',148,icon='folder')
        self.primary(x+1008,y,'Import…',112)
        s.button(x+1184,y-2,'',32,icon='rotate')
        self.quiet(x,y+52,'Project',112,icon='folder')
        s.icon('chevron_right',x+112,y+58,16)
        self.text(x+140,y+71,'assets',64)
        s.icon('chevron_right',x+208,y+58,16)
        self.text(x+236,y+71,'models',112,weight=600)
        self.quiet(x+360,y+52,'Show address',152)
        lx=x+224;dw=280;dx=x+w-dw;gw=dx-lx-24
        s.rect(x,y+100,200,h-112,c['canvas'])
        self.text(x+12,y+128,'FOLDERS',176,12,muted=True,weight=600)
        for i,(label,indent) in enumerate([('Project',0),('assets',16),('models',32),('audio',32),('textures',32)]):
            yy=y+148+i*32
            if label=='models':
                s.rect(x+4,yy,192,28,c['selected'],r=4)
                s.rect(x+4,yy+4,3,20,c['accent'])
            s.icon('folder',x+12+indent,yy+4,18)
            self.text(x+40+indent,yy+19,label,152-indent,weight=600 if label=='models' else 400)
        self.field(lx,y+100,gw-224,'Search folder and subfolders',placeholder=True)
        self.field(lx+gw-212,y+100,112,'All types',enum=True)
        self.quiet(lx+gw-88,y+100,'Tiles',80)
        self.text(lx,y+164,'MODELS',gw,12,muted=True,weight=600)
        items=[('Characters','folder'),('Environment','folder'),('hero.glb','mesh'),('crate.glb','mesh'),('stone.png','h_image'),('hit.wav','h_audio'),('props.glb','mesh'),('notes.txt','h_file')]
        columns=max(1,int(gw//104))
        for i,(label,icon) in enumerate(items):
            tx=lx+(i%columns)*104;ty=y+184+(i//columns)*112
            selected=label=='hero.glb'
            if selected:
                s.rect(tx,ty,96,96,c['selected'],c['accent'],4,2)
                s.icon('h_check',tx+72,ty+8,16,c['accent'])
            s.icon(icon,tx+24,ty+12,48)
            self.text(tx+4,ty+85,label,88,12,weight=600 if selected else 400)
        self.text(lx,y+h-12,'8 items · 1 selected',gw,12,muted=True)
        s.line(dx-12,y+100,dx-12,y+h)
        self.asset_details(dx,y+100,dw)
