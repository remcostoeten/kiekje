export namespace main {
	
	export class AppState {
	    outputPath: string;
	    binds: Record<string, string>;
	
	    static createFrom(source: any = {}) {
	        return new AppState(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.outputPath = source["outputPath"];
	        this.binds = source["binds"];
	    }
	}
	export class CaptureResult {
	    path: string;
	    data: string;
	
	    static createFrom(source: any = {}) {
	        return new CaptureResult(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.path = source["path"];
	        this.data = source["data"];
	    }
	}
	export class ConfigFile {
	    path: string;
	    name: string;
	    preview: string;
	
	    static createFrom(source: any = {}) {
	        return new ConfigFile(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.path = source["path"];
	        this.name = source["name"];
	        this.preview = source["preview"];
	    }
	}
	export class WaybarInfo {
	    height: string;
	    marginTop: string;
	
	    static createFrom(source: any = {}) {
	        return new WaybarInfo(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.height = source["height"];
	        this.marginTop = source["marginTop"];
	    }
	}
	export class Shortcut {
	    source: string;
	    line: number;
	    modifiers: string;
	    key: string;
	    action: string;
	    command: string;
	
	    static createFrom(source: any = {}) {
	        return new Shortcut(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.source = source["source"];
	        this.line = source["line"];
	        this.modifiers = source["modifiers"];
	        this.key = source["key"];
	        this.action = source["action"];
	        this.command = source["command"];
	    }
	}
	export class HyprlandSnapshot {
	    settings: Record<string, string>;
	    shortcuts: Shortcut[];
	    files: ConfigFile[];
	    waybar: WaybarInfo;
	
	    static createFrom(source: any = {}) {
	        return new HyprlandSnapshot(source);
	    }
	
	    constructor(source: any = {}) {
	        if ('string' === typeof source) source = JSON.parse(source);
	        this.settings = source["settings"];
	        this.shortcuts = this.convertValues(source["shortcuts"], Shortcut);
	        this.files = this.convertValues(source["files"], ConfigFile);
	        this.waybar = this.convertValues(source["waybar"], WaybarInfo);
	    }
	
		convertValues(a: any, classs: any, asMap: boolean = false): any {
		    if (!a) {
		        return a;
		    }
		    if (a.slice && a.map) {
		        return (a as any[]).map(elem => this.convertValues(elem, classs));
		    } else if ("object" === typeof a) {
		        if (asMap) {
		            for (const key of Object.keys(a)) {
		                a[key] = new classs(a[key]);
		            }
		            return a;
		        }
		        return new classs(a);
		    }
		    return a;
		}
	}
	

}

