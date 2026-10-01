// 実際の IME(TSF のテキストサービス)で変換の正確さを測る(Google 日本語入力・Microsoft IME・Kotori を
// 同じ条件で比べる。eval/imebench/README.md)。
//
// テキストボックスの窓を前に出し、指定の IME をこのプロセスで有効にして、読みのローマ字をキーとして送り、
// Space(変換)→ Enter(確定)を押して、確定した文字を読む。前の文は渡さない(どの IME にも同じ条件)。
//
// 使い方: ImeBench.exe <テキストサービスの CLSID> <プロファイルの GUID> <入力.tsv> <出力.tsv> [Space の後の待ち ms]
//   入力.tsv: 1 行に「番号<TAB>ローマ字」。出力.tsv: 1 行に「番号<TAB>確定した文字」。
// 計測の間、この PC のキーボード入力を使う(ほかの操作をしない)。
using System;
using System.Collections.Generic;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows.Forms;

[ComImport, Guid("71C6E74C-0F28-11D8-A82A-00065B84435C"), InterfaceType(ComInterfaceType.InterfaceIsIUnknown)]
interface ITfInputProcessorProfileMgr {
  [PreserveSig] int ActivateProfile(uint dwProfileType, ushort langid, ref Guid clsid, ref Guid guidProfile,
                                    IntPtr hkl, uint dwFlags);
}

static class Native {
  [StructLayout(LayoutKind.Sequential)]
  public struct KEYBDINPUT { public ushort wVk; public ushort wScan; public uint dwFlags; public uint time; public IntPtr dwExtraInfo; }
  [StructLayout(LayoutKind.Sequential)]
  public struct INPUT { public uint type; public KEYBDINPUT ki; public long pad; }
  [DllImport("user32.dll", SetLastError = true)] public static extern uint SendInput(uint n, INPUT[] inputs, int size);
  [DllImport("user32.dll")] public static extern short VkKeyScanW(char ch);
  [DllImport("user32.dll")] public static extern uint MapVirtualKeyW(uint code, uint type);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("imm32.dll")] public static extern IntPtr ImmGetContext(IntPtr h);
  [DllImport("imm32.dll")] public static extern bool ImmReleaseContext(IntPtr h, IntPtr c);
  [DllImport("imm32.dll")] public static extern bool ImmSetOpenStatus(IntPtr c, bool open);
  [DllImport("imm32.dll")] public static extern bool ImmSetConversionStatus(IntPtr c, uint conv, uint sentence);
}

class Bench : Form {
  readonly TextBox box = new TextBox();
  // 問ごとにフォーカスを移して戻し、IME が前の問の確定を文脈として持ち越さないようにする。
  readonly TextBox other = new TextBox();
  readonly string[] args;
  public Bench(string[] a) {
    args = a;
    Text = "ImeBench";
    Width = 900; Height = 140;
    TopMost = true;
    box.Dock = DockStyle.Fill;
    box.Font = new System.Drawing.Font("Yu Gothic UI", 14);
    other.Dock = DockStyle.Bottom;
    Controls.Add(box);
    Controls.Add(other);
    Shown += (s, e) => { Activate(); box.Focus(); new Thread(Run) { IsBackground = true }.Start(); };
  }

  static void Key(ushort vk, bool up) {
    var ins = new Native.INPUT[1];
    ins[0].type = 1;
    ins[0].ki.wVk = vk;
    ins[0].ki.wScan = (ushort)Native.MapVirtualKeyW(vk, 0);
    ins[0].ki.dwFlags = up ? 2u : 0u;
    Native.SendInput(1, ins, Marshal.SizeOf(typeof(Native.INPUT)));
  }

  static void Press(ushort vk, bool shift = false) {
    if (shift) Key(0x10, false);
    Key(vk, false);
    Key(vk, true);
    if (shift) Key(0x10, true);
  }

  static void TypeKeys(string romaji, int keyMs) {
    foreach (char ch in romaji) {
      short r = Native.VkKeyScanW(ch);
      if (r == -1) continue;
      Press((ushort)(r & 0xff), (r & 0x100) != 0);
      Thread.Sleep(keyMs);
    }
  }

  void OnUi(Action a) { Invoke(a); }

  void Run() {
    try {
      Guid clsid = new Guid(args[0]), profile = new Guid(args[1]);
      int afterSpace = args.Length > 4 ? int.Parse(args[4]) : 1200;
      OnUi(() => {
        var mgr = (ITfInputProcessorProfileMgr)Activator.CreateInstance(
            Type.GetTypeFromCLSID(new Guid("33C53A50-F456-4884-B049-85FD643ECFED")));
        // TF_PROFILETYPE_INPUTPROCESSOR、日本語、このプロセスだけ・今の入力言語に関係なく
        int hr = mgr.ActivateProfile(1, 0x0411, ref clsid, ref profile, IntPtr.Zero, 0x10000000 | 0x4 | 0x1);
        if (hr != 0) throw new Exception("ActivateProfile 0x" + hr.ToString("x"));
      });
      Thread.Sleep(1500);
      using (var w = new StreamWriter(args[3], false, new UTF8Encoding(false))) {
        foreach (string line in File.ReadAllLines(args[2], Encoding.UTF8)) {
          string[] f = line.Split('\t');
          if (f.Length < 2) continue;
          OnUi(() => { other.Focus(); });
          Thread.Sleep(150);
          OnUi(() => {
            box.Clear();
            Native.SetForegroundWindow(Handle);
            box.Focus();
            IntPtr c = Native.ImmGetContext(box.Handle);
            Native.ImmSetOpenStatus(c, true);
            Native.ImmSetConversionStatus(c, 0x1 | 0x8, 0x8);  // ひらがな・全角、一般
            Native.ImmReleaseContext(box.Handle, c);
          });
          Thread.Sleep(150);
          TypeKeys(f[1], 12);
          Thread.Sleep(250);
          Press(0x20);  // Space(変換)
          Thread.Sleep(afterSpace);
          Press(0x0D);  // Enter(確定)
          Thread.Sleep(250);
          string got = "";
          OnUi(() => { got = box.Text; });
          w.WriteLine(f[0] + "\t" + got.Replace("\t", " ").Replace("\r", "").Replace("\n", ""));
          w.Flush();
        }
      }
    } catch (Exception e) {
      File.WriteAllText(args[3] + ".error.txt", e.ToString());
    }
    OnUi(Close);
  }

  [STAThread]
  static void Main(string[] a) {
    Application.EnableVisualStyles();
    Application.Run(new Bench(a));
  }
}
