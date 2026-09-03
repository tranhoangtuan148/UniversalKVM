import { ExclamationTriangleIcon } from "@radix-ui/react-icons";
import "./Warn.css";

interface Props {
  show: boolean,
  text: string,
}
const WarnText: React.FC<Props> = (props) => {
  const { show, text } = props;

  return (
    <>
      {show && (
        <div className="warn-text">
          <ExclamationTriangleIcon className="warn-icon" />
          <div>{text}</div>
        </div>
      )}
    </>
  );
};

export default WarnText;