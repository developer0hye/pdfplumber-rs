FROM rust:1.98.0-bookworm@sha256:82150a52ec202c1b14d7817e14516c392bb7f5cfebd88f1ed531cb37ebd39922

ARG USERNAME=rustdev
ARG USER_UID=1000
ARG USER_GID=1000

RUN groupadd --gid "${USER_GID}" "${USERNAME}" \
    && useradd --uid "${USER_UID}" --gid "${USER_GID}" \
        --create-home --shell /bin/bash "${USERNAME}"

ENV RUSTUP_HOME=/usr/local/rustup
ENV PATH=/usr/local/cargo/bin:${PATH}

USER rustdev
WORKDIR /workspaces/pdfplumber-rs

CMD ["bash"]
