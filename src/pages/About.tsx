import logo from "/universalkvm.svg";
import "./About.css";

function About() {
  return (
    <div className="about">
      <header className="about-header">
        <img src={logo} alt="" className="about-mark" />
        <div>
          <h1 className="about-title">
            Universal<span className="about-title-accent">KVM</span>
          </h1>
          <p className="about-tagline">
            One keyboard and mouse, driving every computer on your desk.
          </p>
        </div>
      </header>

      <p>
        UniversalKVM sends keyboard and mouse events between computers over your local
        network. Arrange your screens once, and the cursor crosses from one machine to
        the next as if they were a single display.
      </p>

      <div className="eyebrow">Also carries</div>
      <ul className="about-list">
        <li><strong>Clipboard.</strong> Copies across when the cursor moves to another machine.</li>
        <li><strong>Files.</strong> Drag them onto a connected machine to send them.</li>
        <li><strong>Custom borders.</strong> Choose exactly which screen edges connect.</li>
        <li><strong>Per-device capture.</strong> Pick which keyboard or mouse gets shared.</li>
      </ul>

      <div className="eyebrow">Why it exists</div>
      <p>
        It reads the lowest level input events the operating system exposes, so emulated
        keys and movement stay as close to the real thing as possible. It was written
        because few free, open source KVMs work well across Linux, macOS, and Windows at
        once for demanding work such as programming.
      </p>

      <div className="eyebrow">What it cannot do yet</div>
      <div className="about-limits">
        <section>
          <h4>Linux</h4>
          <ul className="about-list">
            <li>The application or your user must be in the <code className="mono">input</code> group.</li>
            <li>On Wayland, borders are unlikely to work.</li>
          </ul>
        </section>

        <section>
          <h4>macOS</h4>
          <ul className="about-list">
            <li>Needs the accessibility permission.</li>
            <li>Needs the local network permission.</li>
          </ul>
        </section>

        <section>
          <h4>Windows</h4>
          <ul className="about-list">
            <li>Most features need admin privileges.</li>
            <li>Dragging files onto the window only works when it runs without admin privileges.</li>
            <li>Working on the login screen or an admin prompt needs admin privileges and PsExec.</li>
            <li>
              Events from keyboards plugged into a Windows machine may not reach other
              machines while this window is not focused, or while a menu is open.
            </li>
          </ul>
        </section>
      </div>

      <div className="eyebrow">Everywhere</div>
      <ul className="about-list">
        <li>The keyboard layout is not translated. Typing on a Canadian English layout and sending to a Canadian French machine produces the Canadian French output.</li>
        <li>Login and lock screens are not fully supported.</li>
      </ul>
    </div>
  );
}

export default About;
