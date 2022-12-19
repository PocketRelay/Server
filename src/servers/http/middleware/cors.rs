use axum::{
    http::{header, HeaderValue, Method, Request, Response},
    response::IntoResponse,
};
use futures_util::future::{ready, BoxFuture};

use tower::{Layer, Service};

pub struct CorsLayer;

impl<S> Layer<S> for CorsLayer {
    type Service = CorsService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        CorsService { inner }
    }
}

pub struct CorsService<S> {
    inner: S,
}

impl<S, B, R> Service<Request<B>> for CorsService<S>
where
    S: Service<Request<B>, Response = Response<R>>,
{
    type Response = EitherResponse<R>;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    #[inline]
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<B>) -> Self::Future {
        if req.method() == Method::OPTIONS {
            let mut res = Response::new("");
            res.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            return Box::pin(async move { Ok(EitherResponse::Options(res)) });
        }

        let res = self.inner.call(req);
        Box::pin(async move {
            let mut res = res.await;
            res.headers_mut().insert(
                header::ACCESS_CONTROL_ALLOW_ORIGIN,
                HeaderValue::from_static("*"),
            );
            Ok(EitherResponse::Normal(res))
        })
    }
}

pub enum EitherResponse<R> {
    Normal(R),
    Options(Response<&'static str>),
}
