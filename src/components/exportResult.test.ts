import { describe, expect, it } from "vitest";
import { terminalExportResult, type ActiveExport } from "./exportResult";

describe("terminal export result", () => {
  const active: ActiveExport = {jobId:"new",outputPath:"C:/out.mp4",progress:0.5,cancelling:false,preset:"balanced",range:{start_frame:30,end_frame:90}};
  it("ignores stale and nonterminal reports",()=>{
    expect(terminalExportResult(active,{jobId:"old",status:"completed",progress:1,error:null})).toBeNull();
    expect(terminalExportResult(active,{jobId:"new",status:"running",progress:0.5,error:null})).toBeNull();
    expect(terminalExportResult(null,{jobId:"new",status:"completed",progress:1,error:null})).toBeNull();
  });
  it("retains the starting settings and terminal error",()=>{
    const result=terminalExportResult(active,{jobId:"new",status:"failed",progress:0.5,error:"disk full"});
    expect(result).toMatchObject({status:"failed",error:"disk full",outputPath:"C:/out.mp4",preset:"balanced",range:{start_frame:30,end_frame:90}});
    expect(terminalExportResult(active,{jobId:"new",status:"cancelled",progress:0,error:null})?.status).toBe("cancelled");
    expect(terminalExportResult(active,{jobId:"new",status:"completed",progress:1,error:null})?.status).toBe("completed");
  });
});
