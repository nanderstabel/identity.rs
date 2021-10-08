// Copyright 2020-2021 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
  any::{Any, TypeId},
  marker::PhantomData,
  pin::Pin,
};

use futures::Future;

use crate::{
  errors::RemoteSendError,
  traits::{ActorRequest, RequestHandler},
  types::RequestContext,
  Actor,
};

#[derive(Clone)]
pub struct AsyncFn<OBJ, REQ, FUT, FUN>
where
  OBJ: 'static,
  REQ: ActorRequest,
  FUT: Future<Output = REQ::Response>,
  FUN: Fn(OBJ, Actor, RequestContext<REQ>) -> FUT,
{
  func: FUN,
  // Need to use the types that appear in the closure's arguments here,
  // as it is otherwise considered unused.
  // Since this type does not actually own any of these types, we use a reference.
  // See also the drop check section in the PhantomData doc.
  _marker_obj: PhantomData<&'static OBJ>,
  _marker_req: PhantomData<&'static REQ>,
}

impl<OBJ, REQ, FUT, FUN> AsyncFn<OBJ, REQ, FUT, FUN>
where
  OBJ: 'static,
  REQ: ActorRequest,
  FUT: Future<Output = REQ::Response>,
  FUN: Fn(OBJ, Actor, RequestContext<REQ>) -> FUT,
{
  pub fn new(func: FUN) -> Self {
    Self {
      func,
      _marker_obj: PhantomData,
      _marker_req: PhantomData,
    }
  }
}

impl<OBJ, REQ, FUT, FUN> RequestHandler for AsyncFn<OBJ, REQ, FUT, FUN>
where
  OBJ: Clone + Send + Sync + 'static,
  REQ: ActorRequest + Send + Sync,
  FUT: Future<Output = REQ::Response> + Send,
  FUN: Send + Sync + Fn(OBJ, Actor, RequestContext<REQ>) -> FUT,
{
  fn invoke<'this>(
    &'this self,
    actor: Actor,
    context: RequestContext<()>,
    object: Box<dyn Any + Send + Sync>,
    request: Box<dyn Any + Send + Sync>,
  ) -> Pin<Box<dyn Future<Output = Box<dyn Any>> + Send + 'this>> {
    let input: Box<REQ> = request.downcast().unwrap();
    let request: RequestContext<REQ> = context.convert(*input);
    let boxed_object: Box<OBJ> = object.downcast().unwrap();
    let future = async move {
      let response: REQ::Response = (self.func)(*boxed_object, actor, request).await;
      let type_erased: Box<dyn Any> = Box::new(response);
      type_erased
    };
    Box::pin(future)
  }

  fn deserialize_request(&self, input: Vec<u8>) -> Result<Box<dyn Any + Send + Sync>, RemoteSendError> {
    log::debug!("Attempt deserialization into {:?}", std::any::type_name::<REQ>());
    let request: REQ = serde_json::from_slice(&input)?;
    Ok(Box::new(request))
  }

  fn object_type_id(&self) -> TypeId {
    TypeId::of::<OBJ>()
  }

  fn clone_object(&self, object: &Box<dyn Any + Send + Sync>) -> Box<dyn Any + Send + Sync> {
    // Double indirection is unfortunately required - the downcast fails otherwise.
    Box::new(object.downcast_ref::<OBJ>().unwrap().clone())
  }
}
