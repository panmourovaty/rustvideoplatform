'use strict';

let zetajs, css;
let context, desktop, xModel, ctrl;

function demo() {
  context = zetajs.getUnoComponentContext();
  desktop = css.frame.Desktop.create(context);

  zetajs.mainPort.onmessage = function(e) {
    switch (e.data.cmd) {
    case 'upload':
      loadFile(e.data.filename);
      break;
    default:
      throw Error('Unknown message command: ' + e.data.cmd);
    }
  };
  zetajs.mainPort.postMessage({cmd: 'thr_running'});
}

function loadFile(filename) {
  const in_path = 'file:///tmp/office/' + filename;
  xModel = desktop.loadComponentFromURL(in_path, '_default', 0, []);
  ctrl = xModel.getCurrentController();
  ctrl.getFrame().getContainerWindow().FullScreen = true;
  zetajs.mainPort.postMessage({cmd: 'ui_ready'});
}

Module.zetajs.then(function(pZetajs) {
  zetajs = pZetajs;
  css = zetajs.uno.com.sun.star;
  demo();
});
