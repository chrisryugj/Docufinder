import { Composition } from 'remotion';
import { IntroVideo } from './IntroVideo';
import { Prologue } from './Prologue';

export const RemotionVideo: React.FC = () => {
  return (
    <>
      <Composition
        id="Prologue"
        component={Prologue}
        durationInFrames={300}
        fps={30}
        width={1920}
        height={1080}
      />
      <Composition
        id="IntroVideo"
        component={IntroVideo}
        durationInFrames={480}
        fps={30}
        width={1920}
        height={1080}
      />
    </>
  );
};
