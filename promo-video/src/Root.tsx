import { Composition } from 'remotion';
import { IntroVideo } from './IntroVideo';

export const RemotionVideo: React.FC = () => {
  return (
    <>
      <Composition
        id="IntroVideo"
        component={IntroVideo}
        durationInFrames={420}
        fps={30}
        width={1920}
        height={1080}
      />
    </>
  );
};
