(function() {var implementors = {};
implementors["identity_core"] = [];
implementors["identity_credential"] = [{"text":"impl&lt;T&gt; TrySignatureMut for VerifiableCredential&lt;T&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T, U&gt; TrySignatureMut for VerifiablePresentation&lt;T, U&gt;","synthetic":false,"types":[]}];
implementors["identity_did"] = [{"text":"impl&lt;T, U, V&gt; TrySignatureMut for Document&lt;Properties&lt;T&gt;, U, V&gt;","synthetic":false,"types":[]},{"text":"impl&lt;T&gt; TrySignatureMut for Properties&lt;T&gt;","synthetic":false,"types":[]}];
implementors["identity_iota"] = [{"text":"impl TrySignatureMut for DocumentDiff","synthetic":false,"types":[]},{"text":"impl TrySignatureMut for Document","synthetic":false,"types":[]}];
if (window.register_implementors) {window.register_implementors(implementors);} else {window.pending_implementors = implementors;}})()