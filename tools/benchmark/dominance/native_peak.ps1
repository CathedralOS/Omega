# Windows keeps process counters on a retained native handle after exit; the
# .NET PeakWorkingSet64 convenience property can no longer query by PID then.
if (-not ('OmegaDominanceProcessResources' -as [type])) {
    Add-Type -TypeDefinition @"
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class OmegaDominanceProcessResources {
    [StructLayout(LayoutKind.Sequential)]
    private struct Counters {
        public uint Size;
        public uint PageFaults;
        public UIntPtr PeakWorkingSet;
        public UIntPtr WorkingSet;
        public UIntPtr QuotaPeakPagedPool;
        public UIntPtr QuotaPagedPool;
        public UIntPtr QuotaPeakNonPagedPool;
        public UIntPtr QuotaNonPagedPool;
        public UIntPtr PagefileUsage;
        public UIntPtr PeakPagefileUsage;
    }
    [DllImport("psapi.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool GetProcessMemoryInfo(SafeProcessHandle process, ref Counters counters, uint size);
    public static long PeakWorkingSetBytes(SafeProcessHandle process) {
        var counters = new Counters();
        counters.Size = (uint)Marshal.SizeOf<Counters>();
        if (!GetProcessMemoryInfo(process, ref counters, counters.Size))
            throw new Win32Exception(Marshal.GetLastWin32Error());
        return checked((long)counters.PeakWorkingSet.ToUInt64());
    }
}
"@
}
