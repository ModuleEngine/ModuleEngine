using System;
using System.Collections.Generic;
using System.ComponentModel;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using Microsoft.Win32.SafeHandles;

public static class QueueJob
{
    private const uint Suspended = 0x4;
    private const uint UseStdHandles = 0x100;
    private const uint KillOnClose = 0x2000;
    private const int ExtendedLimits = 9;
    private const int BasicAccounting = 1;
    private const int BasicProcessIdList = 3;

    [StructLayout(LayoutKind.Sequential)]
    private struct BasicLimits
    {
        public long ProcessTime, JobTime;
        public uint Flags;
        public UIntPtr MinimumWorkingSet, MaximumWorkingSet;
        public uint ActiveProcessLimit;
        public UIntPtr Affinity;
        public uint PriorityClass, SchedulingClass;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Limits
    {
        public BasicLimits Basic;
        public ulong ReadOps, WriteOps, OtherOps, ReadBytes, WriteBytes, OtherBytes;
        public UIntPtr ProcessMemory, JobMemory, PeakProcessMemory, PeakJobMemory;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Accounting
    {
        public long UserTime, KernelTime, PeriodUserTime, PeriodKernelTime;
        public uint PageFaults, TotalProcesses, ActiveProcesses, TerminatedProcesses;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Startup
    {
        public int Size;
        public IntPtr Reserved, Desktop, Title;
        public uint X, Y, Width, Height, Columns, Rows, Fill, Flags;
        public ushort ShowWindow, ReservedSize;
        public IntPtr ReservedData, Input, Output, Error;
    }

    [StructLayout(LayoutKind.Sequential)]
    private struct Child
    {
        public IntPtr Process, Thread;
        public uint ProcessId, ThreadId;
    }

    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern SafeFileHandle CreateJobObject(IntPtr attributes, IntPtr name);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetInformationJobObject(SafeFileHandle job, int kind, ref Limits info, int size);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool QueryInformationJobObject(SafeFileHandle job, int kind, out Accounting info, int size, IntPtr written);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool QueryInformationJobObject(SafeFileHandle job, int kind, IntPtr info, int size, out int written);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool AssignProcessToJobObject(SafeFileHandle job, IntPtr process);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateJobObject(SafeFileHandle job, uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(IntPtr process, uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool TerminateProcess(SafeProcessHandle process, uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool IsProcessInJob(SafeProcessHandle process, SafeFileHandle job, out bool isMember);
    [DllImport("kernel32.dll")]
    private static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool GetExitCodeProcess(IntPtr process, out uint exitCode);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern uint ResumeThread(IntPtr thread);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool SetHandleInformation(SafeFileHandle file, uint mask, uint flags);
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool CloseHandle(IntPtr handle);
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool CreateProcess(string application, StringBuilder command, IntPtr processAttributes,
        IntPtr threadAttributes, bool inheritHandles, uint flags, IntPtr environment, string directory,
        ref Startup startup, out Child child);
    [DllImport("kernel32.dll")]
    private static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")]
    private static extern IntPtr GetThreadDesktop(uint thread);
    [DllImport("user32.dll")]
    private static extern IntPtr OpenInputDesktop(uint flags, bool inherit, uint access);
    [DllImport("user32.dll")]
    private static extern bool CloseDesktop(IntPtr desktop);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)]
    private static extern bool GetUserObjectInformation(IntPtr handle, int index, StringBuilder value, uint length, out uint needed);

    private static void Require(bool success)
    {
        if (!success) throw new Win32Exception(Marshal.GetLastWin32Error());
    }

    private static string DesktopName(IntPtr desktop)
    {
        var name = new StringBuilder(128);
        return desktop != IntPtr.Zero && GetUserObjectInformation(desktop, 2, name, 256, out _)
            ? name.ToString() : null;
    }

    public static bool HasInputDesktop()
    {
        if (!Environment.UserInteractive || Process.GetCurrentProcess().SessionId == 0) return false;
        var input = OpenInputDesktop(0, false, 1);
        if (input == IntPtr.Zero) return false;
        try
        {
            return DesktopName(input) == "Default" && DesktopName(GetThreadDesktop(GetCurrentThreadId())) == "Default";
        }
        finally
        {
            CloseDesktop(input);
        }
    }

    // Quote arguments according to the Windows C runtime argv rules.
    private static string Quote(string arg)
    {
        var result = new StringBuilder("\"");
        int slashes = 0;
        foreach (char c in arg)
        {
            if (c == '\\') { slashes++; continue; }
            result.Append('\\', c == '"' ? slashes * 2 + 1 : slashes);
            result.Append(c);
            slashes = 0;
        }
        return result.Append('\\', slashes * 2).Append('"').ToString();
    }

    private static uint ActiveProcessCount(SafeFileHandle job)
    {
        Require(QueryInformationJobObject(job, BasicAccounting, out var state, Marshal.SizeOf<Accounting>(), IntPtr.Zero));
        return state.ActiveProcesses;
    }

    private static HashSet<int> LiveJobMembers(SafeFileHandle job)
    {
        const int headerSize = 2 * sizeof(int);
        int capacity = headerSize + 32 * IntPtr.Size;
        var buffer = Marshal.AllocHGlobal(capacity);
        try
        {
            if (!QueryInformationJobObject(job, BasicProcessIdList, buffer, capacity, out int written) || written < headerSize)
                return null;
            int listed = Marshal.ReadInt32(buffer, sizeof(int));
            if (listed < 0 || listed != Marshal.ReadInt32(buffer) || listed > (written - headerSize) / IntPtr.Size)
                return null;
            var members = new HashSet<int>();
            for (int i = 0; i < listed; i++)
                members.Add(Marshal.ReadIntPtr(buffer, headerSize + i * IntPtr.Size).ToInt32());
            return members;
        }
        finally
        {
            Marshal.FreeHGlobal(buffer);
        }
    }

    private static void StopLinkerTelemetry(SafeFileHandle job, FileStream log)
    {
        // The MSVC uploader can remain in the job after Cargo and all tests exit.
        // Verify the complete remaining set and leave every normal descendant alone.
        var members = LiveJobMembers(job);
        if (members == null || members.Count == 0) return;
        var uploaders = new List<Process>();
        try
        {
            foreach (int id in members)
            {
                var process = Process.GetProcessById(id);
                uploaders.Add(process);
                if (!string.Equals(process.ProcessName, "vctip", StringComparison.OrdinalIgnoreCase) ||
                    !IsProcessInJob(process.SafeHandle, job, out bool isMember) || !isMember) return;
                var image = process.MainModule;
                if (image == null ||
                    !image.FileName.Contains(@"\VC\Tools\MSVC\", StringComparison.OrdinalIgnoreCase) ||
                    !string.Equals(image.FileVersionInfo.OriginalFilename, "vctip.exe", StringComparison.OrdinalIgnoreCase) ||
                    image.FileVersionInfo.CompanyName != "Microsoft Corporation") return;
            }
            var currentMembers = LiveJobMembers(job);
            if (currentMembers == null || !members.SetEquals(currentMembers)) return;
            foreach (var process in uploaders)
            {
                // Use the same handle that proved membership, even if the PID is later reused.
                if (!TerminateProcess(process.SafeHandle, 0)) continue;
                log.Seek(0, SeekOrigin.End);
                log.Write(Encoding.UTF8.GetBytes($"\r\nQueue closed MSVC VCTIP (PID {process.Id}) after the check exited.\r\n"));
                log.Flush();
            }
        }
        catch (ArgumentException) { /* A listed PID exited before inspection. */ }
        catch (InvalidOperationException) { /* A listed process exited during inspection. */ }
        catch (Win32Exception) { /* Leave unverified processes attached to the deadline. */ }
        finally
        {
            foreach (var process in uploaders) process.Dispose();
        }
    }

    public static int? Run(string exe, string[] args, string cwd, string logPath, TimeSpan timeout)
    {
        using var job = CreateJobObject(IntPtr.Zero, IntPtr.Zero);
        Require(!job.IsInvalid);
        var limits = new Limits { Basic = new BasicLimits { Flags = KillOnClose } };
        Require(SetInformationJobObject(job, ExtendedLimits, ref limits, Marshal.SizeOf<Limits>()));

        using var input = File.OpenRead(@"\\.\NUL");
        using var log = new FileStream(logPath, FileMode.Open, FileAccess.Write, FileShare.ReadWrite);
        log.Seek(0, SeekOrigin.End);
        Require(SetHandleInformation(input.SafeFileHandle, 1, 1));
        Require(SetHandleInformation(log.SafeFileHandle, 1, 1));
        var startup = new Startup
        {
            Size = Marshal.SizeOf<Startup>(),
            Flags = UseStdHandles,
            Input = input.SafeFileHandle.DangerousGetHandle(),
            Output = log.SafeFileHandle.DangerousGetHandle(),
            Error = log.SafeFileHandle.DangerousGetHandle()
        };
        var command = new StringBuilder(Quote(exe));
        foreach (string arg in args) command.Append(' ').Append(Quote(arg));
        Require(CreateProcess(exe, command, IntPtr.Zero, IntPtr.Zero, true, Suspended, IntPtr.Zero, cwd, ref startup, out var child));
        try
        {
            // Attach before resuming so even an immediate grandchild belongs to the job.
            if (!AssignProcessToJobObject(job, child.Process))
            {
                int error = Marshal.GetLastWin32Error();
                TerminateProcess(child.Process, 1);
                throw new Win32Exception(error);
            }
            Require(ResumeThread(child.Thread) != uint.MaxValue);
            var elapsed = Stopwatch.StartNew();
            var nextTelemetryCheck = TimeSpan.Zero;
            bool timedOut = false;
            while (ActiveProcessCount(job) > 0)
            {
                if (!timedOut && elapsed.Elapsed >= timeout)
                {
                    Require(TerminateJobObject(job, 124));
                    timedOut = true;
                }
                else if (!timedOut && elapsed.Elapsed >= nextTelemetryCheck)
                {
                    nextTelemetryCheck = elapsed.Elapsed + TimeSpan.FromSeconds(1);
                    if (WaitForSingleObject(child.Process, 0) == 0) StopLinkerTelemetry(job, log);
                }
                Thread.Sleep(50);
            }
            Require(GetExitCodeProcess(child.Process, out uint code));
            return timedOut ? (int?)null : unchecked((int)code);
        }
        finally
        {
            CloseHandle(child.Thread);
            CloseHandle(child.Process);
        }
    }
}
