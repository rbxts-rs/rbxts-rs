// bench/measure.cs (compiled by bench/release-bench.mjs).
// measure.exe <cwd> <log> <exe> [args...]: runs exe in cwd (stdout+stderr to log), prints one JSON line:
// {"exit":N,"wallMs":..,"cpuMs":..,"peakWsMiB":..,"peakCommitMiB":..}. Peak values read from the process handle after
// exit (GetProcessMemoryInfo), so they are the whole run's peaks. Single process only (roblox-ts and rbxtsc build
// in-process; rbxtsc's watch restart is not used here).
using System;
using System.Diagnostics;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;

static class Measure {
	[StructLayout(LayoutKind.Sequential)]
	struct PMC { public uint cb, PageFaultCount; public UIntPtr PeakWorkingSetSize, WorkingSetSize, QuotaPeakPagedPoolUsage, QuotaPagedPoolUsage, QuotaPeakNonPagedPoolUsage, QuotaNonPagedPoolUsage, PagefileUsage, PeakPagefileUsage; }
	[DllImport("psapi.dll", SetLastError = true)] static extern bool GetProcessMemoryInfo(IntPtr h, out PMC c, uint cb);

	static string Quote(string a) {
		if (a.Length > 0 && a.IndexOfAny(new[] { ' ', '\t', '"' }) < 0) return a;
		var sb = new StringBuilder("\"");
		int bs = 0;
		foreach (var ch in a) {
			if (ch == '\\') { bs++; continue; }
			if (ch == '"') { sb.Append('\\', bs * 2 + 1); bs = 0; sb.Append('"'); continue; }
			sb.Append('\\', bs); bs = 0; sb.Append(ch);
		}
		sb.Append('\\', bs * 2); sb.Append('"');
		return sb.ToString();
	}

	static int Main(string[] argv) {
		var psi = new ProcessStartInfo(argv[2]) { WorkingDirectory = argv[0], UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true, CreateNoWindow = true };
		var args = new StringBuilder();
		for (int i = 3; i < argv.Length; i++) { if (i > 3) args.Append(' '); args.Append(Quote(argv[i])); }
		psi.Arguments = args.ToString();
		using (var log = new StreamWriter(argv[1], false, new UTF8Encoding(false))) {
			var sw = Stopwatch.StartNew();
			var p = Process.Start(psi);
			p.OutputDataReceived += (s, e) => { if (e.Data != null) lock (log) log.WriteLine(e.Data); };
			p.ErrorDataReceived += (s, e) => { if (e.Data != null) lock (log) log.WriteLine(e.Data); };
			p.BeginOutputReadLine(); p.BeginErrorReadLine();
			p.WaitForExit();
			sw.Stop();
			PMC c; GetProcessMemoryInfo(p.Handle, out c, (uint)Marshal.SizeOf(typeof(PMC)));
			Console.WriteLine("{\"exit\":" + p.ExitCode + ",\"wallMs\":" + sw.ElapsedMilliseconds + ",\"cpuMs\":" + (long)p.TotalProcessorTime.TotalMilliseconds +
				",\"peakWsMiB\":" + ((ulong)c.PeakWorkingSetSize / 1048576.0).ToString("F1", System.Globalization.CultureInfo.InvariantCulture) +
				",\"peakCommitMiB\":" + ((ulong)c.PeakPagefileUsage / 1048576.0).ToString("F1", System.Globalization.CultureInfo.InvariantCulture) + "}");
			return 0;
		}
	}
}
