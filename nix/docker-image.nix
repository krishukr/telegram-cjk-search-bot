{ pkgs, bin }:
pkgs.dockerTools.buildLayeredImage {
  name = "telegram-cjk-search-bot";
  tag = "latest";
  contents = [
    bin
    pkgs.dockerTools.caCertificates
  ];
  extraCommands = ''
    ln -s ${bin}/bin app
  '';
  config = {
    Env = [
      "MEILISEARCH_HOST=http://meilisearch:7700"
      "TELOXIDE_TOKEN="
      "RUST_LOG=INFO"
      "TZ=Asia/Shanghai"
    ];
    Cmd = [ "${bin}/bin/bot" ];
  };
}
