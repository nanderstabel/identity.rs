(function() {var implementors = {};
implementors["identity_core"] = [];
implementors["identity_credential"] = [{"text":"impl&lt;T&gt; TrySignature for VerifiableCredential&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T, U&gt; TrySignature for VerifiablePresentation&lt;T, U&gt;","synthetic":false,"types":[]}];
implementors["identity_did"] = [{"text":"impl&lt;T, U, V&gt; TrySignature for Document&lt;Properties&lt;T&gt;, U, V&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T&gt; TrySignature for Properties&lt;T&gt;","synthetic":false,"types":[]}];
implementors["identity_iota"] = [{"text":"impl TrySignature for IotaDocument","synthetic":false,"types":[]},{"text":"impl TrySignature for DocumentDiff","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()